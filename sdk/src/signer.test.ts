import {
  Account,
  BASE_FEE,
  Keypair,
  Networks,
  Operation,
  Transaction,
  TransactionBuilder,
} from "@stellar/stellar-sdk";
import { KeypairSigner, isSigner, toSigner } from "./signer";
import { Signer } from "./types";

describe("KeypairSigner", () => {
  const keypair = Keypair.random();

  it("conforms to the Signer interface", () => {
    const signer: Signer = new KeypairSigner(keypair);
    expect(typeof signer.getPublicKey).toBe("function");
    expect(typeof signer.signTransaction).toBe("function");
  });

  it("getPublicKey resolves to the wrapped keypair's public key", async () => {
    const signer = new KeypairSigner(keypair);
    await expect(signer.getPublicKey()).resolves.toBe(keypair.publicKey());
  });

  it("signTransaction adds a valid signature for the wrapped keypair", async () => {
    const signer = new KeypairSigner(keypair);
    const account = new Account(keypair.publicKey(), "100");
    const unsigned = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: Networks.TESTNET,
    })
      .addOperation(
        Operation.bumpSequence({ bumpTo: "101" }),
      )
      .setTimeout(30)
      .build();

    expect(unsigned.signatures.length).toBe(0);

    const signedXdr = await signer.signTransaction(unsigned.toXDR(), {
      networkPassphrase: Networks.TESTNET,
    });

    const signed = TransactionBuilder.fromXDR(
      signedXdr,
      Networks.TESTNET,
    ) as Transaction;
    expect(signed.signatures.length).toBe(1);

    const hint = keypair.signatureHint();
    const sigHint = signed.signatures[0].hint();
    expect(Buffer.from(sigHint).equals(Buffer.from(hint))).toBe(true);
  });

  it("does not mutate signatures already present in the input XDR when re-signed by a different key", async () => {
    const keypairA = Keypair.random();
    const keypairB = Keypair.random();
    const account = new Account(keypairA.publicKey(), "1");
    const unsigned = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: Networks.TESTNET,
    })
      .addOperation(Operation.bumpSequence({ bumpTo: "2" }))
      .setTimeout(30)
      .build();

    const signerA = new KeypairSigner(keypairA);
    const oneSig = await signerA.signTransaction(unsigned.toXDR(), {
      networkPassphrase: Networks.TESTNET,
    });

    const signerB = new KeypairSigner(keypairB);
    const twoSigsXdr = await signerB.signTransaction(oneSig, {
      networkPassphrase: Networks.TESTNET,
    });

    const twoSigs = TransactionBuilder.fromXDR(
      twoSigsXdr,
      Networks.TESTNET,
    ) as Transaction;
    expect(twoSigs.signatures.length).toBe(2);
  });
});

describe("isSigner", () => {
  it("returns true for objects implementing getPublicKey + signTransaction", () => {
    const fake: Signer = {
      getPublicKey: async () => "G...",
      signTransaction: async (xdr) => xdr,
    };
    expect(isSigner(fake)).toBe(true);
  });

  it("returns false for a Keypair (it has no getPublicKey/signTransaction methods)", () => {
    expect(isSigner(Keypair.random())).toBe(false);
  });

  it("returns false for null, undefined, and plain objects", () => {
    expect(isSigner(null)).toBe(false);
    expect(isSigner(undefined)).toBe(false);
    expect(isSigner({})).toBe(false);
  });
});

describe("toSigner", () => {
  it("wraps a Keypair in a KeypairSigner", () => {
    const keypair = Keypair.random();
    const signer = toSigner(keypair);
    expect(signer).toBeInstanceOf(KeypairSigner);
  });

  it("passes an existing Signer through unchanged", () => {
    const fake: Signer = {
      getPublicKey: async () => "G...",
      signTransaction: async (xdr) => xdr,
    };
    expect(toSigner(fake)).toBe(fake);
  });

  it("throws for a value that is neither a Keypair nor a Signer", () => {
    expect(() => toSigner({} as unknown as Signer)).toThrow(
      /Invalid caller/,
    );
  });
});
