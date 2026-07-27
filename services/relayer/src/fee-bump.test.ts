import {
  Account,
  BASE_FEE,
  FeeBumpTransaction,
  Keypair,
  Networks,
  Operation,
  Transaction,
  TransactionBuilder,
} from "@stellar/stellar-sdk";
import {
  buildSignedFeeBumpTransaction,
  decodeInnerTransaction,
  InvalidInnerTransactionError,
} from "./fee-bump";

const NETWORK_PASSPHRASE = Networks.TESTNET;

/** Build a real, signed inner transaction for a given player keypair. */
function buildInnerTransactionXdr(playerKeypair: Keypair, sequence = "100"): string {
  const account = new Account(playerKeypair.publicKey(), sequence);
  const tx = new TransactionBuilder(account, {
    fee: "0",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      Operation.manageData({
        name: "gasless-relay-test",
        value: "sponsor-me",
      }),
    )
    .setTimeout(30)
    .build();

  tx.sign(playerKeypair);
  return tx.toXDR();
}

describe("decodeInnerTransaction", () => {
  it("decodes a valid signed transaction", () => {
    const player = Keypair.random();
    const xdr = buildInnerTransactionXdr(player);

    const decoded = decodeInnerTransaction(xdr, NETWORK_PASSPHRASE);
    expect(decoded.source).toBe(player.publicKey());
    expect(decoded.signatures.length).toBe(1);
  });

  it("rejects garbage XDR", () => {
    expect(() => decodeInnerTransaction("not-valid-xdr-at-all", NETWORK_PASSPHRASE)).toThrow(
      InvalidInnerTransactionError,
    );
  });

  it("rejects an unsigned transaction", () => {
    const player = Keypair.random();
    const account = new Account(player.publicKey(), "100");
    const tx = new TransactionBuilder(account, {
      fee: "0",
      networkPassphrase: NETWORK_PASSPHRASE,
    })
      .addOperation(Operation.manageData({ name: "x", value: "y" }))
      .setTimeout(30)
      .build();

    expect(() => decodeInnerTransaction(tx.toXDR(), NETWORK_PASSPHRASE)).toThrow(
      /must be signed/,
    );
  });

  it("rejects a fee-bump transaction passed as the inner transaction", () => {
    const player = Keypair.random();
    const sponsor = Keypair.random();
    const innerXdr = buildInnerTransactionXdr(player);
    const innerTx = TransactionBuilder.fromXDR(innerXdr, NETWORK_PASSPHRASE);
    if (innerTx instanceof FeeBumpTransaction) {
      throw new Error("test setup error: expected a plain Transaction");
    }
    const feeBump = TransactionBuilder.buildFeeBumpTransaction(
      sponsor,
      BASE_FEE,
      innerTx as Transaction,
      NETWORK_PASSPHRASE,
    );

    expect(() => decodeInnerTransaction(feeBump.toXDR(), NETWORK_PASSPHRASE)).toThrow(
      /must not itself be a fee-bump/,
    );
  });
});

describe("buildSignedFeeBumpTransaction", () => {
  it("wraps a signed inner transaction in a signed fee-bump envelope", () => {
    const player = Keypair.random();
    const sponsor = Keypair.random();
    const innerXdr = buildInnerTransactionXdr(player);

    const feeBumpTx = buildSignedFeeBumpTransaction({
      innerTransactionXdr: innerXdr,
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
    });

    // The sponsor is the fee source, not the player.
    expect(feeBumpTx.feeSource).toBe(sponsor.publicKey());

    // The wrapped inner transaction is exactly what the player signed.
    expect(feeBumpTx.innerTransaction.source).toBe(player.publicKey());
    expect(feeBumpTx.innerTransaction.toXDR()).toBe(
      TransactionBuilder.fromXDR(innerXdr, NETWORK_PASSPHRASE).toXDR(),
    );

    // The sponsor's signature is on the OUTER envelope, not forged onto
    // the inner transaction (which still carries only the player's sig).
    expect(feeBumpTx.signatures.length).toBe(1);
    expect(feeBumpTx.innerTransaction.signatures.length).toBe(1);

    // Fee is a positive integer string (exact arithmetic is the SDK's
    // concern; we just assert it charged something sane).
    expect(Number(feeBumpTx.fee)).toBeGreaterThan(0);
  });

  it("respects a custom base fee", () => {
    const player = Keypair.random();
    const sponsor = Keypair.random();
    const innerXdr = buildInnerTransactionXdr(player);

    const cheap = buildSignedFeeBumpTransaction({
      innerTransactionXdr: innerXdr,
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      baseFee: "100",
    });
    const expensive = buildSignedFeeBumpTransaction({
      innerTransactionXdr: innerXdr,
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      baseFee: "10000",
    });

    expect(Number(expensive.fee)).toBeGreaterThan(Number(cheap.fee));
  });

  it("propagates decode errors for an invalid inner transaction", () => {
    const sponsor = Keypair.random();
    expect(() =>
      buildSignedFeeBumpTransaction({
        innerTransactionXdr: "garbage",
        sponsorKeypair: sponsor,
        networkPassphrase: NETWORK_PASSPHRASE,
      }),
    ).toThrow(InvalidInnerTransactionError);
  });
});
