// use ethers::{
//     middleware::{transformer::{Transformer, TransformerError}, SignerMiddleware}, prelude::*, providers::{Http, Provider}, signers::{LocalWallet, Signer as EthersSigner}, types::transaction::eip2718::TypedTransaction,
// };
// use eyre::Result;
// use middleware::EIP2771GasRelayerTransformer;
// use std::sync::Arc;
// use alloy::{signers::{local::PrivateKeySigner, Signer}, sol_types::eip712_domain};

// mod abi {
//     use ethers::contract::abigen;

//     // Generate the type-safe contract bindings using the JSON ABI
//     abigen!(
//         CounterByAddress,
//         "../blockchain/artifacts/contracts/CounterByAddress.sol/CounterByAddress.json"
//     );
//     abigen!(
//         Forwarder,
//         "../blockchain/artifacts/contracts/Forwarder.sol/Forwarder.json"
//     );
// }

// mod middleware {
//     use alloy::{signers::{local::PrivateKeySigner, Signer, SignerSync}, sol_types::eip712_domain};
//     use ethers::{core::k256::ecdsa::SigningKey, middleware::transformer::{Transformer, TransformerError}, providers::Middleware, types::{transaction::eip2718::TypedTransaction, Bytes, Eip1559TransactionRequest, NameOrAddress, U256}, utils::{id, secret_key_to_address}};
//     use tokio::runtime::Runtime;

//     use crate::abi;

//     pub mod alloy_structs {
//         use alloy::sol;
//         use serde::Serialize;

//         sol! {
//             #[derive(Debug, Serialize)]
//             struct ForwardRequest {
//                 address from;
//                 address to;
//                 uint256 value;
//                 uint256 gas;
//                 uint256 nonce;
//                 bytes data;
//             }
//         }
//     }

//     #[derive(Debug)]
//     pub struct EIP2771GasRelayerTransformer<M> {
//         transaction_signer: SigningKey,
//         forwarder_with_gas_signer: abi::Forwarder<M>,
//     }

//     impl<M> EIP2771GasRelayerTransformer<M> {
//         pub fn new(transaction_signer: SigningKey, forwarder_with_gas_signer: abi::Forwarder<M>) -> Self {
//             Self { transaction_signer, forwarder_with_gas_signer }
//         }
//     }

//     impl<M> Transformer for EIP2771GasRelayerTransformer<M>
//     where
//         M: Middleware,
//     {
//         fn transform(&self, tx: &mut TypedTransaction) -> Result<(), TransformerError> {
//             match tx {
//                 TypedTransaction::Eip1559(tx_req) => {
//                     let transaction_signer_address = secret_key_to_address(&self.transaction_signer);

//                     // We require the callsite to set the nonce.
//                     let nonce = tx_req.nonce.expect("Nonce is not set").to_owned();

//                     // Get the signature over the typed data
//                     // Here, we use alloy to generate the typed data signature because there is a bug in ethers-rs that causes
//                     // the encoding for the data field (Bytes) to be incorrect.
//                     let signature = {
//                         let alloy_domain = eip712_domain! {
//                             name: "GSNv2 Forwarder",
//                             version: "0.0.1",
//                             chain_id: 31337,
//                             verifying_contract: alloy::primitives::Address::new(self.forwarder_with_gas_signer.address().0),
//                         };

//                         let alloy_struct = alloy_structs::ForwardRequest {
//                             from: alloy::primitives::Address::new(transaction_signer_address.0),
//                             to: alloy::primitives::Address::new(tx_req.to.clone().expect("To is not set").as_address().expect("To is not an address").0),
//                             value: alloy::primitives::U256::from_limbs(U256::from(tx_req.value.unwrap_or(U256::from(0))).0),
//                             gas: alloy::primitives::U256::from(30000), // TODO: May wish to use dynamic
//                             nonce: alloy::primitives::U256::from_limbs(nonce.0),
//                             data: alloy::primitives::Bytes::from(tx_req.data.clone().expect("Data is not set").to_vec()),
//                         };

//                         // Use the meta wallet to sign the request
//                         let meta_signer: PrivateKeySigner = PrivateKeySigner::from_signing_key(self.transaction_signer.clone());
//                         let alloy_sig = meta_signer.sign_typed_data_sync(&alloy_struct, &alloy_domain).expect("Failed to sign typed data");
//                         Bytes::from(alloy_sig.as_bytes())
//                     };

//                     let forwarder_execute_req = abi::forwarder::ForwardRequest {
//                         from: transaction_signer_address,
//                         to: tx_req.to.clone().expect("To is not set").as_address().expect("To is not an address").to_owned(),
//                         value: tx_req.value.unwrap_or(U256::from(0)),
//                         gas: U256::from(30000), // TODO: May wish to use dynamic
//                         nonce,
//                         data: tx_req.data.clone().expect("Data is not set"),
//                     };

//                     // let selector = id("execute((address,address,uint256,uint256,uint256,bytes),bytes)");
//                     let encoded_data = self.forwarder_with_gas_signer.execute(forwarder_execute_req, signature).calldata();

//                     // TODO: Set various things on the TX object.

//                     let to = tx_req.to.clone().expect("To is not set").as_address().expect("To is not an address").to_owned();

//                     // tx_req.from = Some(transaction_signer_address);
//                     tx_req.to = Some(NameOrAddress::from(self.forwarder_with_gas_signer.address()));
//                     tx_req.value = Some(tx_req.value.unwrap_or(U256::from(0)));
//                     tx_req.gas = Some(U256::from(30000)); // TODO: May wish to use dynamic
//                     tx_req.data = encoded_data;
//                     // We used the nonce field at the callsite for specifying the nonce for the transaction signer, so here
//                     // we need to reset the nonce for this signer.
//                     self.inner().get_transaction_count(from, block);
//                     println!("Tx {:?}", tx_req);
//                 }
//                 // TODO: Do not panic, return error instead.
//                 _ => panic!("Unsupported transaction type"),
//             };

//             Ok(())
//         }
//     }
// }

// #[tokio::main]
// async fn main() -> Result<()> {
//     // Connect to the network (using local hardhat node by default)
//     let provider = Provider::<Http>::try_from("http://localhost:8545")?;
//     let chain_id = provider.get_chainid().await?;
//     println!("Chain ID: {}", chain_id);

//     // Create a new wallet with no funds. This wallet will be the transaction signer.
//     let meta_wallet = LocalWallet::new(&mut rand::thread_rng()).with_chain_id(chain_id.as_u64());

//     // Get the counter value for this wallet
//     let counter_address = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512".parse::<Address>()?;
//     let counter_read = abi::CounterByAddress::new(counter_address, Arc::new(provider.clone()));

//     let meta_wallet_counter = counter_read.get_counter(meta_wallet.address()).call().await?;
//     println!("Counter value for {}: {}", meta_wallet.address(), meta_wallet_counter);

//     let forwarder_address = "0x5FbDB2315678afecb367f032d93F642f64180aa3".parse::<Address>()?;

//     let gas_client = {
//         // Load private key from environment variable
//         let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string();
//         let gas_wallet = private_key.parse::<LocalWallet>()?;
//         let gas_wallet = gas_wallet.with_chain_id(chain_id.as_u64());

//         // Create a client
//         let gas_client = SignerMiddleware::new(provider.clone(), gas_wallet);

//         Arc::new(gas_client)
//     };

//     let forwarder_with_gas_signer = abi::forwarder::Forwarder::new(forwarder_address, gas_client);

//     let meta_client = {
//         let meta_client = SignerMiddleware::new(provider.clone(), meta_wallet.clone());
//         let meta_signer = meta_wallet.signer().clone();

//         println!("Meta wallet chain id {:?}", meta_wallet.chain_id());

//         let eip_2771_transformer = EIP2771GasRelayerTransformer::new(meta_signer, forwarder_with_gas_signer.clone());

//         Arc::new(TransformerMiddleware::new(meta_client, eip_2771_transformer))
//     };

//     let gas_client_with_forwarder = {
//         // Load private key from environment variable
//         let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string();
//         let gas_wallet = private_key.parse::<LocalWallet>()?;
//         let gas_wallet = gas_wallet.with_chain_id(chain_id.as_u64());

//         // Create a client
//         let gas_client = SignerMiddleware::new(provider.clone(), gas_wallet);

//         let meta_signer = meta_wallet.signer().clone();
//         let eip_2771_transformer = EIP2771GasRelayerTransformer::new(meta_signer, forwarder_with_gas_signer.clone());

//         let gas_client_with_forwarder = TransformerMiddleware::new(gas_client, eip_2771_transformer);

//         Arc::new(gas_client_with_forwarder)
//     };

//     // let counter_write = abi::CounterByAddress::new(counter_address, meta_client);
//     let counter_write = abi::CounterByAddress::new(counter_address, gas_client_with_forwarder);

//     // Get the nonce for the transaction signer
//     let nonce = forwarder_with_gas_signer.get_nonce(meta_wallet.address()).call().await.expect("Failed to get nonce");
//     println!("Nonce: {:?}", nonce);

//     println!("Sending transaction to counter");
//     let fn_call = counter_write.increment().nonce(nonce);
//     let tx = fn_call.send().await;
//     println!("Transaction sent! Waiting for confirmation...");
//     match tx {
//         Err(e) => {
//             println!("Error: {:?}", e);
//         }
//         Ok(tx) => {
//             let receipt = tx.await?;
//             println!("Transaction confirmed: {:?}", receipt);
//         }
//     }

//     // Get the counter value
//     let meta_wallet_counter = counter_read.get_counter(meta_wallet.address()).call().await?;
//     println!("Counter value for {}: {}", meta_wallet.address(), meta_wallet_counter);

//     Ok(())
// }

fn main() {
    todo!()
}
