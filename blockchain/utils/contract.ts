import { Forwarder } from "../typechain-types";

export async function sendMetaTransaction(
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
    gas: 30000,
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
