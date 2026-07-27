import { Keypair } from "@stellar/stellar-sdk";
import { ContractClient } from "./contract-client";
import { buildSignedFeeBumpTransaction, decodeInnerTransaction, InvalidInnerTransactionError } from "./fee-bump";
import { logRelayEvent } from "./logger";
import { validateRelayRequest } from "./validation";
import { RateLimiter, RelayRequest, RelayResult } from "./types";

export interface RelayerManagerConfig {
  contractClient: ContractClient;
  sponsorKeypair: Keypair;
  networkPassphrase: string;
  /** Per-operation base fee for the fee-bump envelope. Defaults to the SDK's BASE_FEE. */
  baseFee?: string;
  ipRateLimiter: RateLimiter;
  addressRateLimiter: RateLimiter;
}

/**
 * Orchestrates one `/relay` request end to end:
 *
 *   1. validate request shape
 *   2. enforce rate limits (per source IP, per claimed player address)
 *   3. decode + sanity-check the inner transaction (must be signed, must
 *      not itself be a fee-bump, source must match the claimed player)
 *   4. on-chain eligibility pre-check against `gas_sponsor.rs` (bot
 *      detection fraud gate, one-time cap, daily/lifetime/per-user caps,
 *      fund balance) — BEFORE any fee-bump is built or submitted
 *   5. build + sign the fee-bump transaction
 *   6. submit it
 *
 * Every step logs a structured event via `logRelayEvent` for
 * fraud-monitoring visibility, whether the outcome is a rejection, a
 * submission, or a failure.
 */
export class RelayerManager {
  constructor(private readonly config: RelayerManagerConfig) {}

  async relay(request: RelayRequest, sourceIp: string): Promise<RelayResult> {
    const playerAddress =
      typeof (request as Partial<RelayRequest>)?.playerAddress === "string"
        ? request.playerAddress
        : undefined;

    logRelayEvent({ event: "relay_attempt", playerAddress, sourceIp });

    const validation = validateRelayRequest(request);
    if (!validation.valid) {
      const detail = validation.errors.join("; ");
      logRelayEvent({
        event: "relay_rejected",
        playerAddress,
        sourceIp,
        reason: "INVALID_REQUEST",
        detail,
      });
      return { status: "rejected", reason: "INVALID_REQUEST", detail };
    }

    // From here on, TS knows the shape is valid; narrow it explicitly.
    const validRequest = request as RelayRequest;

    const ipLimit = await this.config.ipRateLimiter.checkAndRecord(`ip:${sourceIp}`);
    if (!ipLimit.allowed) {
      const detail = "per-IP rate limit exceeded";
      logRelayEvent({
        event: "relay_rejected",
        playerAddress: validRequest.playerAddress,
        sourceIp,
        reason: "RATE_LIMITED",
        detail,
        retryAfterMs: ipLimit.retryAfterMs,
      });
      return { status: "rejected", reason: "RATE_LIMITED", detail };
    }

    const addressLimit = await this.config.addressRateLimiter.checkAndRecord(
      `addr:${validRequest.playerAddress}`,
    );
    if (!addressLimit.allowed) {
      const detail = "per-address rate limit exceeded";
      logRelayEvent({
        event: "relay_rejected",
        playerAddress: validRequest.playerAddress,
        sourceIp,
        reason: "RATE_LIMITED",
        detail,
        retryAfterMs: addressLimit.retryAfterMs,
      });
      return { status: "rejected", reason: "RATE_LIMITED", detail };
    }

    let innerTx;
    try {
      innerTx = decodeInnerTransaction(
        validRequest.innerTransactionXdr,
        this.config.networkPassphrase,
      );
    } catch (err) {
      const detail = err instanceof InvalidInnerTransactionError ? err.message : String(err);
      logRelayEvent({
        event: "relay_rejected",
        playerAddress: validRequest.playerAddress,
        sourceIp,
        reason: "INVALID_TRANSACTION_ENVELOPE",
        detail,
      });
      return { status: "rejected", reason: "INVALID_TRANSACTION_ENVELOPE", detail };
    }

    if (innerTx.source !== validRequest.playerAddress) {
      const detail = `inner transaction source (${innerTx.source}) does not match claimed playerAddress (${validRequest.playerAddress})`;
      logRelayEvent({
        event: "relay_rejected",
        playerAddress: validRequest.playerAddress,
        sourceIp,
        reason: "SOURCE_ACCOUNT_MISMATCH",
        detail,
      });
      return { status: "rejected", reason: "SOURCE_ACCOUNT_MISMATCH", detail };
    }

    const eligibility = await this.config.contractClient.checkEligibility(
      validRequest.playerAddress,
    );
    if (!eligibility.eligible) {
      logRelayEvent({
        event: "relay_rejected",
        playerAddress: validRequest.playerAddress,
        sourceIp,
        reason: "SPONSORSHIP_INELIGIBLE",
        detail: eligibility.message,
        code: eligibility.code,
      });
      return {
        status: "rejected",
        reason: "SPONSORSHIP_INELIGIBLE",
        detail: eligibility.message,
      };
    }

    logRelayEvent({
      event: "relay_eligible",
      playerAddress: validRequest.playerAddress,
      sourceIp,
    });

    let feeBumpTx;
    try {
      feeBumpTx = buildSignedFeeBumpTransaction({
        innerTransactionXdr: validRequest.innerTransactionXdr,
        sponsorKeypair: this.config.sponsorKeypair,
        networkPassphrase: this.config.networkPassphrase,
        baseFee: this.config.baseFee,
      });
    } catch (err) {
      const detail = err instanceof Error ? err.message : String(err);
      logRelayEvent({
        event: "relay_failed",
        playerAddress: validRequest.playerAddress,
        sourceIp,
        reason: "fee_bump_build_failed",
        detail,
      });
      return { status: "failed", reason: detail };
    }

    try {
      const result = await this.config.contractClient.submit(feeBumpTx);
      const feeBumpHash = feeBumpTx.hash().toString("hex");
      logRelayEvent({
        event: "relay_submitted",
        playerAddress: validRequest.playerAddress,
        sourceIp,
        hash: result.hash,
        feeBumpHash,
      });
      return { status: "submitted", hash: result.hash, feeBumpHash };
    } catch (err) {
      const detail = err instanceof Error ? err.message : String(err);
      logRelayEvent({
        event: "relay_failed",
        playerAddress: validRequest.playerAddress,
        sourceIp,
        reason: "submission_failed",
        detail,
      });
      return { status: "failed", reason: detail };
    }
  }
}
