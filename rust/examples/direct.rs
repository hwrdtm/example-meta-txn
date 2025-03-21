use ethers::{
    middleware::SignerMiddleware,
    prelude::*,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
};
use eyre::Result;
use std::sync::Arc;

// Generate the type-safe contract bindings using the JSON ABI
abigen!(
    CounterByAddress,
    "../blockchain/artifacts/contracts/CounterByAddress.sol/CounterByAddress.json"
);

#[tokio::main]
async fn main() -> Result<()> {
    // Load private key from environment variable
    let private_key =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string();
    let wallet = private_key.parse::<LocalWallet>()?;

    // Connect to the network (using local hardhat node by default)
    let provider = Provider::<Http>::try_from("http://localhost:8545")?;
    let chain_id = provider.get_chainid().await?;
    let wallet = wallet.with_chain_id(chain_id.as_u64());

    // Create a client
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);

    // Contract address - replace with your deployed contract address
    let contract_address = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512".parse::<Address>()?;
    let contract = CounterByAddress::new(contract_address, client.clone());

    // Send the increment transaction
    println!("Sending increment transaction...");
    let fn_call = contract.increment();
    let tx = fn_call.send().await?;

    println!("Transaction sent! Waiting for confirmation...");
    let receipt = tx.await?;
    println!("Transaction confirmed: {:?}", receipt);

    // Get the counter value
    let signer_address = client.address();
    let counter = contract.get_counter(signer_address).call().await?;
    println!("Counter value for {}: {}", signer_address, counter);

    // Call the definitelyReverts function
    let fn_call = contract.definitely_reverts();
    let tx = fn_call.send().await;

    match tx {
        Err(e) => {
            println!("Error: {:?}", e);
        }
        Ok(tx) => {
            println!("Tx: {:?}", tx);
        }
    }

    Ok(())
}
