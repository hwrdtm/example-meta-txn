use ethers::{
    middleware::SignerMiddleware,
    prelude::*,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer as EthersSigner},
};
use eyre::Result;
use middleware::EIP2771GasRelayerMiddleware;
use std::sync::Arc;

mod abi {
    use ethers::contract::abigen;

    // Generate the type-safe contract bindings using the JSON ABI
    abigen!(
        CounterByAddress,
        "../blockchain/artifacts/contracts/CounterByAddress.sol/CounterByAddress.json"
    );
    abigen!(
        Forwarder,
        "../blockchain/artifacts/contracts/Forwarder.sol/Forwarder.json"
    );
}

mod middleware {
    use alloy::{
        signers::{local::PrivateKeySigner, Signer},
        sol_types::eip712_domain,
    };
    use async_trait::async_trait;
    use ethers::{
        core::k256::ecdsa::SigningKey,
        providers::{Middleware, MiddlewareError, PendingTransaction},
        types::{
            transaction::eip2718::TypedTransaction, BlockId, Bytes, Eip1559TransactionRequest, U256,
        },
        utils::secret_key_to_address,
    };
    use thiserror::Error;

    use crate::abi;

    pub mod alloy_structs {
        use alloy::sol;
        use serde::Serialize;

        sol! {
            #[derive(Debug, Serialize)]
            struct ForwardRequest {
                address from;
                address to;
                uint256 value;
                uint256 gas;
                uint256 nonce;
                bytes data;
            }
        }
    }

    #[derive(Debug)]
    pub struct EIP2771GasRelayerMiddleware<M> {
        inner: M,
        /// This is the signer that will sign the meta-transaction. This is NOT the signer that
        /// will send the transaction to the Forwarder contract.
        transaction_signer: SigningKey,
        forwarder_with_gas_signer: abi::Forwarder<M>,
    }

    impl<M> EIP2771GasRelayerMiddleware<M> {
        pub fn new(
            inner: M,
            transaction_signer: SigningKey,
            forwarder_with_gas_signer: abi::Forwarder<M>,
        ) -> Self {
            Self {
                inner,
                transaction_signer,
                forwarder_with_gas_signer,
            }
        }
    }

    #[derive(Error, Debug)]
    pub enum EIP2771GasRelayerMiddlewareError<M: Middleware> {
        #[error("{0}")]
        SignerError(String),

        #[error("{0}")]
        MiddlewareError(M::Error),

        #[error("{0}")]
        ContractRevert(String),

        #[error("Failed to get nonce")]
        FailedToGetNonce(String),

        #[error("Failed to estimate gas")]
        FailedToEstimateGas(String),

        #[error("Missing chain ID")]
        MissingChainID(String),

        #[error("Missing to address")]
        MissingToAddress,

        #[error("Missing data")]
        MissingData,

        #[error("Conversion error")]
        ConversionError(String),

        #[error("Unsupported transaction type")]
        UnsupportedTransactionType,
    }

    impl<M> MiddlewareError for EIP2771GasRelayerMiddlewareError<M>
    where
        M: Middleware,
    {
        type Inner = M::Error;

        fn from_err(src: M::Error) -> Self {
            EIP2771GasRelayerMiddlewareError::MiddlewareError(src)
        }

        fn as_inner(&self) -> Option<&Self::Inner> {
            match self {
                EIP2771GasRelayerMiddlewareError::MiddlewareError(e) => Some(e),
                _ => None,
            }
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl<M> Middleware for EIP2771GasRelayerMiddleware<M>
    where
        M: Middleware,
    {
        type Error = EIP2771GasRelayerMiddlewareError<M>;
        type Provider = M::Provider;
        type Inner = M;

        fn inner(&self) -> &M {
            &self.inner
        }

        async fn send_transaction<Tx: Into<TypedTransaction> + Send + Sync>(
            &self,
            tx: Tx,
            block: Option<BlockId>,
        ) -> Result<PendingTransaction<'_, Self::Provider>, Self::Error> {
            // Get the nonce for the transaction signer
            let transaction_signer_address = secret_key_to_address(&self.transaction_signer);
            let nonce = self
                .forwarder_with_gas_signer
                .get_nonce(transaction_signer_address)
                .call()
                .await
                .map_err(|e| EIP2771GasRelayerMiddlewareError::FailedToGetNonce(e.to_string()))?;

            let typed_tx = tx.into();

            // Estimate the gas needed for the transaction.
            let gas = self
                .inner()
                .estimate_gas(&typed_tx, block)
                .await
                .map_err(|e| {
                    EIP2771GasRelayerMiddlewareError::FailedToEstimateGas(e.to_string())
                })?;

            let typed_tx: Eip1559TransactionRequest = match typed_tx {
                TypedTransaction::Eip1559(tx) => tx,
                _ => return Err(EIP2771GasRelayerMiddlewareError::UnsupportedTransactionType),
            };

            // Get the signature over the typed data
            // Here, we use alloy to generate the typed data signature because there is a bug in ethers-rs that causes
            // the encoding for the data field (Bytes) to be incorrect.
            let signature = {
                let chain_id = {
                    match typed_tx.chain_id {
                        Some(chain_id) => chain_id.as_u64(),
                        None => {
                            let chain_id = self.inner().get_chainid().await.map_err(|e| {
                                EIP2771GasRelayerMiddlewareError::MissingChainID(e.to_string())
                            })?;
                            chain_id.as_u64()
                        }
                    }
                };

                let alloy_domain = eip712_domain! {
                    name: "GSNv2 Forwarder",
                    version: "0.0.1",
                    chain_id: chain_id,
                    verifying_contract: alloy::primitives::Address::new(self.forwarder_with_gas_signer.address().0),
                };

                let alloy_struct = alloy_structs::ForwardRequest {
                    from: alloy::primitives::Address::new(transaction_signer_address.0),
                    to: alloy::primitives::Address::new(
                        typed_tx
                            .to
                            .clone()
                            .ok_or(EIP2771GasRelayerMiddlewareError::MissingToAddress)?
                            .as_address()
                            .ok_or(EIP2771GasRelayerMiddlewareError::ConversionError(
                                "To is not an address".to_string(),
                            ))?
                            .0,
                    ),
                    value: alloy::primitives::U256::from_limbs(
                        U256::from(typed_tx.value.unwrap_or(U256::from(0))).0,
                    ),
                    gas: alloy::primitives::U256::from_limbs(gas.0),
                    nonce: alloy::primitives::U256::from_limbs(U256::from(nonce).0),
                    data: alloy::primitives::Bytes::from(
                        typed_tx
                            .data
                            .clone()
                            .ok_or(EIP2771GasRelayerMiddlewareError::MissingData)?
                            .to_vec(),
                    ),
                };

                // Use the meta wallet to sign the request
                let meta_signer: PrivateKeySigner =
                    PrivateKeySigner::from_signing_key(self.transaction_signer.clone());
                let alloy_sig = meta_signer
                    .sign_typed_data(&alloy_struct, &alloy_domain)
                    .await
                    .map_err(|e| EIP2771GasRelayerMiddlewareError::SignerError(e.to_string()))?;
                Bytes::from(alloy_sig.as_bytes())
            };

            let forwarder_execute_req = abi::forwarder::ForwardRequest {
                from: transaction_signer_address,
                to: typed_tx
                    .to
                    .ok_or(EIP2771GasRelayerMiddlewareError::MissingToAddress)?
                    .as_address()
                    .ok_or(EIP2771GasRelayerMiddlewareError::ConversionError(
                        "To is not an address".to_string(),
                    ))?
                    .to_owned(),
                value: typed_tx.value.unwrap_or(U256::from(0)),
                gas,
                nonce,
                data: typed_tx
                    .data
                    .ok_or(EIP2771GasRelayerMiddlewareError::MissingData)?,
            };

            let fn_call = self
                .forwarder_with_gas_signer
                .execute(forwarder_execute_req, signature);
            let tx = fn_call.send().await;

            match tx {
                Err(e) => {
                    match e
                        .decode_contract_revert::<abi::forwarder::ForwarderErrors>()
                        .ok_or(EIP2771GasRelayerMiddlewareError::ConversionError(
                            "Failed to decode contract revert".to_string(),
                        ))? {
                        abi::forwarder::ForwarderErrors::SignatureDoesNotMatch(chain_e) => {
                            return Err(EIP2771GasRelayerMiddlewareError::ContractRevert(
                                chain_e.to_string(),
                            ));
                        }
                        _ => {
                            return Err(EIP2771GasRelayerMiddlewareError::ContractRevert(
                                e.to_string(),
                            ));
                        }
                    }
                }
                Ok(tx) => Ok(PendingTransaction::new(
                    tx.tx_hash(),
                    self.inner().provider(),
                )),
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to the network (using local hardhat node by default)
    let provider = Provider::<Http>::try_from("http://localhost:8545")?;
    let chain_id = provider.get_chainid().await?;

    // Create a new wallet with no funds. This wallet will be the transaction signer.
    let meta_wallet = LocalWallet::new(&mut rand::thread_rng());

    // Get the counter value for this wallet
    let counter_address = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512".parse::<Address>()?;
    let counter_read = abi::CounterByAddress::new(counter_address, Arc::new(provider.clone()));

    let meta_wallet_counter = counter_read
        .get_counter(meta_wallet.address())
        .call()
        .await?;
    println!(
        "Counter value for {}: {}",
        meta_wallet.address(),
        meta_wallet_counter
    );

    let forwarder_address = "0x5FbDB2315678afecb367f032d93F642f64180aa3".parse::<Address>()?;

    let gas_client = {
        // Load private key from environment variable
        let private_key =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string();
        let gas_wallet = private_key.parse::<LocalWallet>()?;
        let gas_wallet = gas_wallet.with_chain_id(chain_id.as_u64());

        // Create a client
        let gas_client = SignerMiddleware::new(provider.clone(), gas_wallet);

        Arc::new(gas_client)
    };

    let forwarder_with_gas_signer = abi::forwarder::Forwarder::new(forwarder_address, gas_client);

    let meta_client = {
        let meta_signer = meta_wallet.signer().clone();
        let meta_client =
            SignerMiddleware::new(provider.clone(), LocalWallet::from(meta_signer.clone()));

        Arc::new(EIP2771GasRelayerMiddleware::new(
            meta_client,
            meta_signer,
            forwarder_with_gas_signer,
        ))
    };

    let counter_write = abi::CounterByAddress::new(counter_address, meta_client);

    println!("Sending transaction to counter");
    let fn_call = counter_write.increment();
    let tx = fn_call.send().await;
    println!("Transaction sent! Waiting for confirmation...");
    match tx {
        Err(e) => {
            println!("Error: {:?}", e);
        }
        Ok(tx) => {
            let receipt = tx.await?;
            println!("Transaction confirmed: {:?}", receipt);
        }
    }

    // Get the counter value
    let meta_wallet_counter = counter_read
        .get_counter(meta_wallet.address())
        .call()
        .await?;
    println!(
        "Counter value for {}: {}",
        meta_wallet.address(),
        meta_wallet_counter
    );

    Ok(())
}
