import {
  Account,
  Keypair,
  Networks,
  StrKey,
  Transaction,
  TransactionBuilder,
} from "@stellar/stellar-sdk";

const mockGetAccount = jest.fn();
const mockSendTransaction = jest.fn();
const mockGetTransaction = jest.fn();
const mockSimulateTransaction = jest.fn();

jest.mock("@stellar/stellar-sdk", () => {
  const actual = jest.requireActual("@stellar/stellar-sdk");
  return {
    ...actual,
    SorobanRpc: {
      ...actual.SorobanRpc,
      Server: jest.fn().mockImplementation(() => ({
        getAccount: mockGetAccount,
        sendTransaction: mockSendTransaction,
        getTransaction: mockGetTransaction,
        simulateTransaction: mockSimulateTransaction,
      })),
    },
  };
});

import { StellarNebulaClient } from "./client";
import { Signer, ShipType, TransactionSignOptions } from "./types";

const CONTRACT_ID = StrKey.encodeContract(Buffer.alloc(32, 1));

/** Mimics a remote wallet (e.g. WalletConnect): it owns a real Keypair, but
 *  is only exposed to the client through the Signer interface — proving the
 *  client doesn't require a Keypair instance, just Signer-shaped behavior. */
class FakeRemoteSigner implements Signer {
  constructor(private readonly keypair: Keypair) {}

  async getPublicKey(): Promise<string> {
    return this.keypair.publicKey();
  }

  async signTransaction(
    xdr: string,
    opts: TransactionSignOptions,
  ): Promise<string> {
    const transaction = TransactionBuilder.fromXDR(
      xdr,
      opts.networkPassphrase,
    ) as Transaction;
    transaction.sign(this.keypair);
    return transaction.toXDR();
  }
}

function makeClient(): StellarNebulaClient {
  return new StellarNebulaClient({
    contractId: CONTRACT_ID,
    networkPassphrase: Networks.TESTNET,
    rpcUrl: "http://localhost:8000/soroban/rpc",
  });
}

beforeEach(() => {
  jest.clearAllMocks();
});

describe("StellarNebulaClient — Keypair caller (existing behavior)", () => {
  it("mints a ship using a raw Keypair, unchanged from before", async () => {
    const keypair = Keypair.random();
    mockGetAccount.mockResolvedValue(new Account(keypair.publicKey(), "10"));
    mockSendTransaction.mockResolvedValue({
      status: "PENDING",
      hash: "keypair-hash",
    });
    mockGetTransaction.mockResolvedValue({
      status: "SUCCESS",
      returnValue: "1",
    });

    const client = makeClient();
    const result = await client.mintShip(
      keypair,
      keypair.publicKey(),
      ShipType.Explorer,
    );

    expect(result.success).toBe(true);
    expect(result.txHash).toBe("keypair-hash");

    const submitted = mockSendTransaction.mock.calls[0][0] as Transaction;
    expect(submitted.signatures.length).toBe(1);
    expect(
      Buffer.from(submitted.signatures[0].hint()).equals(
        Buffer.from(keypair.signatureHint()),
      ),
    ).toBe(true);
  });
});

describe("StellarNebulaClient — Signer caller (new capability)", () => {
  it("mints a ship using an arbitrary Signer-conforming object", async () => {
    const remoteKeypair = Keypair.random();
    const remoteSigner = new FakeRemoteSigner(remoteKeypair);

    mockGetAccount.mockResolvedValue(
      new Account(remoteKeypair.publicKey(), "42"),
    );
    mockSendTransaction.mockResolvedValue({
      status: "PENDING",
      hash: "signer-hash",
    });
    mockGetTransaction.mockResolvedValue({
      status: "SUCCESS",
      returnValue: "7",
    });

    const client = makeClient();
    const result = await client.mintShip(
      remoteSigner,
      remoteKeypair.publicKey(),
      ShipType.Fighter,
    );

    expect(result.success).toBe(true);
    expect(result.txHash).toBe("signer-hash");
    expect(mockGetAccount).toHaveBeenCalledWith(remoteKeypair.publicKey());

    const submitted = mockSendTransaction.mock.calls[0][0] as Transaction;
    expect(submitted.signatures.length).toBe(1);
    expect(
      Buffer.from(submitted.signatures[0].hint()).equals(
        Buffer.from(remoteKeypair.signatureHint()),
      ),
    ).toBe(true);
  });

  it("returns a failure result (not a throw) for an invalid caller", async () => {
    const client = makeClient();
    const result = await client.mintShip(
      {} as unknown as Signer,
      "GABC",
      ShipType.Miner,
    );

    expect(result.success).toBe(false);
    expect(result.error).toMatch(/Invalid caller/);
    expect(mockGetAccount).not.toHaveBeenCalled();
  });

  it("surfaces a failed-transaction status without throwing", async () => {
    const keypair = Keypair.random();
    mockGetAccount.mockResolvedValue(new Account(keypair.publicKey(), "1"));
    mockSendTransaction.mockResolvedValue({ status: "ERROR", hash: "bad" });

    const client = makeClient();
    const result = await client.mintShip(
      keypair,
      keypair.publicKey(),
      ShipType.Trader,
    );

    expect(result.success).toBe(false);
    expect(result.error).toBe("Transaction failed");
  });
});
