import {
  BASE_FEE,
  FeeBumpTransaction,
  Keypair,
  Transaction,
  TransactionBuilder,
} from "@stellar/stellar-sdk";

export class InvalidInnerTransactionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "InvalidInnerTransactionError";
  }
}

/**
 * Decode a base64 XDR envelope into a `Transaction`, rejecting anything
 * that isn't a plain (non-fee-bump) transaction. This is the "inner
 * transaction" a player's wallet builds and signs — the relayer never
 * builds contract-call operations itself, it only wraps what the client
 * already produced.
 */
export function decodeInnerTransaction(
  innerTransactionXdr: string,
  networkPassphrase: string,
): Transaction {
  let parsed;
  try {
    parsed = TransactionBuilder.fromXDR(innerTransactionXdr, networkPassphrase);
  } catch (err) {
    throw new InvalidInnerTransactionError(
      `could not decode inner transaction XDR: ${(err as Error).message}`,
    );
  }

  if (parsed instanceof FeeBumpTransaction) {
    throw new InvalidInnerTransactionError(
      "inner transaction must not itself be a fee-bump transaction",
    );
  }

  if (parsed.signatures.length === 0) {
    throw new InvalidInnerTransactionError(
      "inner transaction must be signed by the player's account before relaying",
    );
  }

  return parsed;
}

export interface BuildFeeBumpParams {
  innerTransactionXdr: string;
  sponsorKeypair: Keypair;
  networkPassphrase: string;
  /** Per-operation base fee for the fee-bump envelope, in stroops. Defaults to `BASE_FEE`. */
  baseFee?: string;
}

/**
 * Build and sign a real Stellar fee-bump transaction wrapping the
 * player's already-signed inner transaction. This is the actual Stellar
 * primitive for third-party fee sponsorship (`TransactionBuilder
 * .buildFeeBumpTransaction`) — the sponsor account pays the network fee,
 * the inner transaction's operations execute exactly as the player signed
 * them, under the player's own authorization.
 *
 * Pure and network-free: takes an XDR string in, returns a signed
 * `FeeBumpTransaction` out. Fully unit-testable without touching a live
 * network — submission is a separate step (see `contract-client.ts`).
 */
export function buildSignedFeeBumpTransaction(
  params: BuildFeeBumpParams,
): FeeBumpTransaction {
  const innerTx = decodeInnerTransaction(
    params.innerTransactionXdr,
    params.networkPassphrase,
  );

  const feeBumpTx = TransactionBuilder.buildFeeBumpTransaction(
    params.sponsorKeypair,
    params.baseFee ?? BASE_FEE,
    innerTx,
    params.networkPassphrase,
  );

  feeBumpTx.sign(params.sponsorKeypair);

  return feeBumpTx;
}
