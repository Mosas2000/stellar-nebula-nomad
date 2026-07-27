import { Keypair, TransactionBuilder } from "@stellar/stellar-sdk";
import { Signer, TransactionSignOptions } from "./types";

/**
 * Adapts a raw Stellar `Keypair` to the `Signer` interface, so
 * `StellarNebulaClient` can treat a local keypair and a remote signer
 * (WalletConnect, hardware wallet, etc.) identically.
 */
export class KeypairSigner implements Signer {
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
    );
    transaction.sign(this.keypair);
    return transaction.toXDR();
  }
}

/** Narrows a value to the `Signer` interface via structural duck-typing. */
export function isSigner(value: unknown): value is Signer {
  return (
    !!value &&
    typeof value === "object" &&
    typeof (value as Signer).getPublicKey === "function" &&
    typeof (value as Signer).signTransaction === "function"
  );
}

/** Normalizes a `Keypair | Signer` into a `Signer`. */
export function toSigner(caller: Keypair | Signer): Signer {
  if (caller instanceof Keypair) {
    return new KeypairSigner(caller);
  }
  if (isSigner(caller)) {
    return caller;
  }
  throw new Error(
    "Invalid caller: expected a Keypair or an object implementing the Signer interface " +
      "(getPublicKey(): Promise<string>, signTransaction(xdr, opts): Promise<string>).",
  );
}
