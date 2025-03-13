import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import hre from "hardhat";
import { Forwarder } from "../typechain-types";

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

async function sendMetaTransaction(
  ethers: any,
  txnData: any,
  metaTransactionSigner: any,
  forwarderContractWithFundedWallet: Forwarder,
  recipientContractAddress: string
): Promise<string> {
  // Get the balance of the metaTransactionSigner before the meta-txn
  const metaTransactionSignerBalanceBefore = await ethers.provider.getBalance(
    await metaTransactionSigner.getAddress()
  );

  // Get the nonce from the forwarder
  const nonce = await forwarderContractWithFundedWallet.getNonce(
    await metaTransactionSigner.getAddress()
  );

  const gasLimit = await ethers.provider.estimateGas({
    ...txnData,
    from: await metaTransactionSigner.getAddress(),
  });

  // Construct the EIP-2771 request
  const metaTxn = {
    from: await metaTransactionSigner.getAddress(),
    to: recipientContractAddress,
    value: "0",
    gas: gasLimit.toString(),
    nonce: nonce.toString(),
    data: txnData.data,
  };

  // Create domain for EIP-712 signing, which needs to match the forwarder contract
  const domain = {
    name: "GSNv2 Forwarder",
    version: "0.0.1",
    chainId: await ethers.provider
      .getNetwork()
      .then((network: any) => network.chainId),
    verifyingContract: await forwarderContractWithFundedWallet.getAddress(),
  };

  const types = {
    ForwardRequest: [
      { name: "from", type: "address" },
      { name: "to", type: "address" },
      { name: "value", type: "uint256" },
      { name: "gas", type: "uint256" },
      { name: "nonce", type: "uint256" },
      { name: "data", type: "bytes" },
    ],
  };

  // Sign the meta-txn with typed data using the ephemeral wallet
  const signature = await metaTransactionSigner.signTypedData(
    domain,
    types,
    metaTxn
  );

  // Execute the txn
  const tx = await forwarderContractWithFundedWallet.execute(
    metaTxn,
    signature
  );

  // Now that the meta-txn has been executed, we need to assert that the metaTransactionSigner
  // has not had their balance changed to help prove the function of meta-txn (it should not use
  // any of the funds in the metaTransactionSigner's balance).
  const metaTransactionSignerBalanceAfter = await ethers.provider.getBalance(
    await metaTransactionSigner.getAddress()
  );
  if (
    metaTransactionSignerBalanceAfter !== metaTransactionSignerBalanceBefore
  ) {
    throw new Error("Meta-txn signer balance changed");
  }

  return tx.hash;
}
