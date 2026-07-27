/**
 * Shared types for the gasless transaction relayer.
 *
 * See README.md for the full `/relay` request/response contract.
 */

/** Body accepted by `POST /relay`. */
export interface RelayRequest {
  /**
   * Base64-encoded XDR of a `TransactionEnvelope` for the user's inner
   * transaction. This transaction must already be fully built (correct
   * sequence number for the player's account) and signed by the player's
   * own keypair. Its `fee` field is irrelevant — the relayer never charges
   * it; only the fee-bump envelope's fee is actually paid, by the sponsor
   * account.
   */
  innerTransactionXdr: string;
  /**
   * The Stellar account (G...) requesting sponsorship. Must exactly match
   * the source account of the decoded inner transaction — this binds the
   * eligibility check to the account that will actually execute the
   * transaction, so a request can't claim sponsorship for one address
   * while relaying a transaction sourced from another.
   */
  playerAddress: string;
}

/** Outcome of a `/relay` call, returned to the caller. */
export type RelayResult =
  | { status: "submitted"; hash: string; feeBumpHash: string }
  | { status: "rejected"; reason: RejectionReason; detail?: string }
  | { status: "failed"; reason: string };

export type RejectionReason =
  | "INVALID_REQUEST"
  | "RATE_LIMITED"
  | "INVALID_TRANSACTION_ENVELOPE"
  | "SOURCE_ACCOUNT_MISMATCH"
  | "SPONSORSHIP_INELIGIBLE";

/** Result of validating the shape of an incoming RelayRequest. */
export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

/** Result of a sliding-window rate-limit check. */
export interface RateLimitResult {
  allowed: boolean;
  remaining: number;
  /** Milliseconds until the caller may retry, only set when `allowed` is false. */
  retryAfterMs?: number;
}

/** A pluggable rate-limit backend (in-memory or Redis-backed). */
export interface RateLimiter {
  checkAndRecord(key: string): Promise<RateLimitResult>;
}

/**
 * Mirrors `SponsorError` in `src/gas_sponsor.rs`. Kept in sync manually —
 * this is the contract boundary between the Rust contract and this
 * TypeScript service, so any change to the Rust enum's discriminants must
 * be reflected here.
 */
export enum SponsorErrorCode {
  AlreadySponsored = 1,
  DailyCapReached = 2,
  InsufficientFunds = 3,
  Unauthorized = 4,
  ProfileNotVerified = 5,
  InvalidAmount = 6,
  NotInitialized = 7,
  PerUserCapReached = 8,
  PerUserDailyCapReached = 9,
  SuspiciousActivity = 10,
}

export const SPONSOR_ERROR_MESSAGES: Record<SponsorErrorCode, string> = {
  [SponsorErrorCode.AlreadySponsored]: "player has already used their one-time sponsorship",
  [SponsorErrorCode.DailyCapReached]: "global daily sponsorship cap reached",
  [SponsorErrorCode.InsufficientFunds]: "sponsorship fund balance is too low",
  [SponsorErrorCode.Unauthorized]: "caller is not authorized",
  [SponsorErrorCode.ProfileNotVerified]: "player profile is not verified",
  [SponsorErrorCode.InvalidAmount]: "invalid amount specified",
  [SponsorErrorCode.NotInitialized]: "sponsorship system is not initialized",
  [SponsorErrorCode.PerUserCapReached]: "player's lifetime sponsorship cap reached",
  [SponsorErrorCode.PerUserDailyCapReached]: "player's daily sponsorship cap reached",
  [SponsorErrorCode.SuspiciousActivity]:
    "player is flagged as high-risk by bot detection (CAPTCHA required)",
};

/** Outcome of the on-chain eligibility pre-check. */
export type EligibilityResult =
  | { eligible: true }
  | { eligible: false; code?: SponsorErrorCode; message: string };

/** A single structured log entry for a relay attempt. */
export interface RelayLogEntry {
  timestamp: string;
  event:
    | "relay_attempt"
    | "relay_rejected"
    | "relay_eligible"
    | "relay_submitted"
    | "relay_failed";
  playerAddress?: string;
  sourceIp?: string;
  reason?: string;
  detail?: string;
  hash?: string;
  feeBumpHash?: string;
  [key: string]: unknown;
}
