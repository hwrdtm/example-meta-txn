# Meta Transaction Example

1. Spin up Anvil with `anvil -a 10`.
2. Deploy the contracts using `npx hardhat run scripts/deploy.ts --network localhost`
3. Run Rust using the corresponding command for the use case:
  - Direction transaction: `cargo run --example direct`
  - Meta transaction: `cargo run --example meta`
  - Meta transaction with Ethers middleware: `cargo run --example meta-middleware`
