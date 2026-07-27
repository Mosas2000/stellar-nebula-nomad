import { StrKey } from "@stellar/stellar-sdk";
import { ValidationResult } from "./types";

/**
 * Validate the shape of an incoming `/relay` request body before any
 * network or on-chain work happens. Deliberately conservative: reject
 * anything ambiguous rather than guess.
 */
export function validateRelayRequest(body: unknown): ValidationResult {
  const errors: string[] = [];

  if (typeof body !== "object" || body === null) {
    return { valid: false, errors: ["request body must be a JSON object"] };
  }

  const { innerTransactionXdr, playerAddress } = body as Record<string, unknown>;

  if (typeof innerTransactionXdr !== "string" || innerTransactionXdr.trim().length === 0) {
    errors.push("innerTransactionXdr is required and must be a non-empty string");
  }

  if (typeof playerAddress !== "string" || playerAddress.trim().length === 0) {
    errors.push("playerAddress is required and must be a non-empty string");
  } else if (!StrKey.isValidEd25519PublicKey(playerAddress)) {
    errors.push("playerAddress must be a valid Stellar account public key (starts with G)");
  }

  return { valid: errors.length === 0, errors };
}
