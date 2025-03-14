import { ethers } from "hardhat";
import { sendMetaTransaction } from "../utils/contract";
import hre from "hardhat";

async function main() {
  // Load a wallet using private key
  const meta_wallet = new ethers.Wallet(
    "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ethers.provider
  );

  const counter = await ethers.getContractAt(
    "CounterByAddress",
    "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"
  );

  let metaWalletCounter = await counter.getCounter(meta_wallet.address);
  console.log(`Counter value for ${meta_wallet.address}: ${metaWalletCounter}`);

  const txnData = await counter
    .connect(meta_wallet)
    .increment.populateTransaction();

  const gasClient = new ethers.Wallet(
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    ethers.provider
  );
  const forwarderContractWithFundedWallet = await ethers.getContractAt(
    "Forwarder",
    "0x5FbDB2315678afecb367f032d93F642f64180aa3",
    gasClient
  );

  await sendMetaTransaction(
    hre.ethers,
    txnData,
    meta_wallet,
    forwarderContractWithFundedWallet,
    await counter.getAddress()
  );

  // Get the counter value
  metaWalletCounter = await counter.getCounter(meta_wallet.address);
  console.log(`Counter value for ${meta_wallet.address}: ${metaWalletCounter}`);
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
