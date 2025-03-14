import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import hre from "hardhat";
import { sendMetaTransaction } from "../utils/contract";

describe("CounterByAddress", function () {
  async function deployFixture() {
    const [owner, otherAccount] = await hre.ethers.getSigners();

    const Forwarder = await hre.ethers.getContractFactory("Forwarder");
    const forwarder = await Forwarder.deploy();

    const CounterByAddress = await hre.ethers.getContractFactory(
      "CounterByAddress"
    );
    const counter = await CounterByAddress.deploy();

    // set the trusted forwarder address
    await counter.setTrustedForwarderAddress(await forwarder.getAddress());

    return { counter, owner, otherAccount, forwarder };
  }

  it("should increment the counter for the given address", async function () {
    const { counter, owner, otherAccount } = await loadFixture(deployFixture);

    await counter.increment();
    expect(await counter.getCounter(owner.address)).to.equal(1);
  });

  it("should increment the counter using EIP-2771 meta txns", async function () {
    const { counter, owner, otherAccount, forwarder } = await loadFixture(
      deployFixture
    );

    // Create brand new wallet without funds
    const brandNewWallet = hre.ethers.Wallet.createRandom();

    const txnData = await counter
      .connect(brandNewWallet)
      .increment.populateTransaction();

    const forwarderContractWithFundedWallet = forwarder.connect(owner);

    await sendMetaTransaction(
      hre.ethers,
      txnData,
      brandNewWallet,
      forwarderContractWithFundedWallet,
      await counter.getAddress()
    );

    expect(await counter.getCounter(brandNewWallet.address)).to.equal(1);
  });
});
