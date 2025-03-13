import { ethers } from "hardhat";

async function main() {
  console.log("Deploying CounterByAddress...");

  const Counter = await ethers.getContractFactory("CounterByAddress");
  const counter = await Counter.deploy();
  await counter.waitForDeployment();

  const address = await counter.getAddress();
  console.log(`CounterByAddress deployed to: ${address}`);
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
