import { Keypair } from "@stellar/stellar-sdk";

export const MISSING_SPONSOR_SECRET_MESSAGE =
  "SPONSOR_SECRET_KEY not configured. Fund a Stellar account to act as the fee-bump sponsor and set this env var before starting the relayer.";

export class MissingSponsorSecretError extends Error {
  constructor() {
    super(MISSING_SPONSOR_SECRET_MESSAGE);
    this.name = "MissingSponsorSecretError";
  }
}

/**
 * Load and validate the sponsor account's secret key at startup.
 *
 * This is the account that pays every fee-bump's network fee, so the
 * service must fail fast — never start in a silently-broken state — if
 * it's missing or malformed. `Keypair.fromSecret` is left to throw
 * naturally on a malformed key (wrong prefix, length, or checksum)
 * rather than us hand-rolling secret-key format validation.
 */
export function loadSponsorKeypair(secretEnvVar: string | undefined): Keypair {
  if (!secretEnvVar || secretEnvVar.trim().length === 0) {
    throw new MissingSponsorSecretError();
  }

  // Let Keypair.fromSecret throw its own descriptive error for a
  // malformed secret (bad prefix, length, or checksum) — no placeholder,
  // no swallowed failure.
  return Keypair.fromSecret(secretEnvVar);
}
