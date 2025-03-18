use alloy::{
    signers::{local::PrivateKeySigner, Signer},
    sol_types::eip712_domain,
};
use ethers::{
    middleware::SignerMiddleware,
    prelude::*,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer as EthersSigner},
};
use eyre::Result;
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

mod alloy_structs {
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

    let txn_data = {
        let fn_call = counter_read.increment();
        fn_call.calldata().expect("Failed to get calldata")
    };

    // Get the nonce for this address
    let forwarder_address = "0x5FbDB2315678afecb367f032d93F642f64180aa3".parse::<Address>()?;
    let forwarder_read = abi::Forwarder::new(forwarder_address, Arc::new(provider.clone()));

    let nonce = forwarder_read
        .get_nonce(meta_wallet.address())
        .call()
        .await?;

    // Here, we use alloy to generate the typed data signature because there is a bug in ethers-rs that causes
    // the encoding for the data field (Bytes) to be incorrect.
    let signature = {
        let alloy_domain = eip712_domain! {
            name: "GSNv2 Forwarder",
            version: "0.0.1",
            chain_id: 31337,
            verifying_contract: alloy::primitives::Address::new(forwarder_address.0),
        };

        let alloy_struct = alloy_structs::ForwardRequest {
            from: alloy::primitives::Address::new(meta_wallet.address().0),
            to: alloy::primitives::Address::new(counter_address.0),
            value: alloy::primitives::U256::from(0),
            gas: alloy::primitives::U256::from(30000),
            nonce: alloy::primitives::U256::from_limbs(U256::from(nonce).0),
            data: alloy::primitives::Bytes::from(txn_data.to_vec()),
        };

        // Use the meta wallet to sign the request
        let signer = meta_wallet.signer();
        let meta_signer: PrivateKeySigner = PrivateKeySigner::from_signing_key(signer.clone());
        let alloy_sig = meta_signer
            .sign_typed_data(&alloy_struct, &alloy_domain)
            .await?;
        Bytes::from(alloy_sig.as_bytes())
    };

    let forwarder_execute_req = abi::forwarder::ForwardRequest {
        from: meta_wallet.address(),
        to: counter_address,
        value: U256::from(0),
        gas: U256::from(30000),
        nonce,
        data: txn_data,
    };

    let gas_client = {
        // Load private key from environment variable
        let private_key =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string();
        let gas_wallet = private_key.parse::<LocalWallet>()?;
        let gas_wallet = gas_wallet.with_chain_id(chain_id.as_u64());

        // Create a client
        let gas_client = SignerMiddleware::new(provider, gas_wallet);
        Arc::new(gas_client)
    };

    // Contract address - replace with your deployed contract address
    let forwarder_write = abi::forwarder::Forwarder::new(forwarder_address, gas_client.clone());

    // Send the increment transaction via the Forwarder
    println!("Sending increment meta-transaction...");
    let fn_call = forwarder_write.execute(forwarder_execute_req, signature);
    let tx = fn_call.send().await;
    println!("Transaction sent! Waiting for confirmation...");
    match tx {
        Err(e) => {
            match e
                .decode_contract_revert::<abi::forwarder::ForwarderErrors>()
                .expect("Failed to decode contract revert")
            {
                abi::forwarder::ForwarderErrors::SignatureDoesNotMatch(_) => {
                    println!("Signature does not match");
                }
                _ => {
                    println!("Unknown error");
                }
            }
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
