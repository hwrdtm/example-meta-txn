import { ethers } from "hardhat";

async function main() {
  console.log("Deploying CounterByAddress...");

  const Forwarder = await ethers.getContractFactory("Forwarder");
  const forwarder = await Forwarder.deploy();
  await forwarder.waitForDeployment();

  const forwarderAddress = await forwarder.getAddress();
  console.log(`Forwarder deployed to: ${forwarderAddress}`);

  const Counter = await ethers.getContractFactory("CounterByAddress");
  const counter = await Counter.deploy();
  await counter.waitForDeployment();

  const counterAddress = await counter.getAddress();
  console.log(`CounterByAddress deployed to: ${counterAddress}`);

  // Set the trusted forwarder address for the counter
  await counter.setTrustedForwarderAddress(forwarderAddress);
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
