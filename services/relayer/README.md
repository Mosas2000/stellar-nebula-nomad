# Gasless Transaction Relayer

Metatransaction relayer for Stellar Nebula Nomad. A player builds and signs
their own contract-call transaction with no fee they intend to pay; this
service checks their on-chain sponsorship eligibility, then wraps that
transaction in a real Stellar **fee-bump transaction** paid for by a
sponsor account, and submits it. The player never needs XLM to interact
with the game for the actions the contract's gas-sponsorship system
covers.

## How it works

Stellar has a native primitive for exactly this: a
[fee-bump transaction](https://developers.stellar.org/docs/encyclopedia/fee-bump-transactions).
The player signs an ordinary transaction (their "inner transaction") citing
their own account and sequence number, with whatever fee they like (`0` is
fine — it's never charged). The relayer wraps that transaction in a
`FeeBumpTransaction` envelope, sourced and signed by the sponsor account.
The network charges the sponsor's account for the fee; the inner
transaction's operations execute exactly as the player authorized them,
under the player's own signature. Nothing about the inner transaction's
authorization changes — the relayer cannot make the player's account do
anything they didn't sign.

This is why the relayer never constructs contract-call operations itself:
the player's own wallet builds the real operation (e.g. a call into
`scan_nebula` or `sponsor_first_scan`), signs it, and the relayer's only
job is fee sponsorship plus fraud/eligibility gating.

## `POST /relay`

### Request body

```json
{
  "innerTransactionXdr": "AAAAAgAAAAA...",
  "playerAddress": "GABC...XYZ"
}
```

- `innerTransactionXdr` — base64 XDR of a `TransactionEnvelope` for the
  player's transaction. Must already be built with the player's real
  sequence number and signed by the player's own keypair. It must **not**
  itself be a fee-bump transaction.
- `playerAddress` — the Stellar account (`G...`) requesting sponsorship.
  Must exactly match the inner transaction's source account — this binds
  the eligibility check to the account that will actually execute it.

### Responses

| status | HTTP | body |
|---|---|---|
| Accepted and submitted | 200 | `{ "status": "submitted", "hash": "...", "feeBumpHash": "..." }` |
| Rejected: bad request shape | 400 | `{ "status": "rejected", "reason": "INVALID_REQUEST", "detail": "..." }` |
| Rejected: undecodable/invalid envelope | 400 | `{ "status": "rejected", "reason": "INVALID_TRANSACTION_ENVELOPE", "detail": "..." }` |
| Rejected: inner tx source ≠ playerAddress | 400 | `{ "status": "rejected", "reason": "SOURCE_ACCOUNT_MISMATCH", "detail": "..." }` |
| Rejected: rate-limited | 429 | `{ "status": "rejected", "reason": "RATE_LIMITED", "detail": "..." }` |
| Rejected: on-chain eligibility denied | 403 | `{ "status": "rejected", "reason": "SPONSORSHIP_INELIGIBLE", "detail": "..." }` |
| Failed: network/submission error | 502 | `{ "status": "failed", "reason": "..." }` |

`SPONSORSHIP_INELIGIBLE`'s `detail` mirrors one of `SponsorError`'s
variants in `src/gas_sponsor.rs` (already-sponsored, daily cap reached,
insufficient funds, profile not verified, per-user caps reached, or
**flagged as suspicious by bot detection** — the fraud-detection gate).

### Processing order

1. **Request validation** — shape and field checks only, no network calls.
2. **Rate limiting** — per source IP and per claimed `playerAddress`,
   sliding window (defense in depth; the real caps live on-chain).
3. **Inner transaction decode + sanity check** — must parse, must be
   signed, must not be a fee-bump itself, source must equal
   `playerAddress`.
4. **On-chain eligibility pre-check** — simulates a call into
   `gas_sponsor.rs`'s `check_sponsorship_eligibility(player)` view
   function via `SorobanRpc.Server.simulateTransaction`. This never
   commits a ledger write and costs nothing; the relayer never builds a
   real fee-bump transaction for a request that would just fail on-chain.
   This is also where the bot-detection fraud gate lives: an address
   flagged by `bot_detection`'s suspicion scoring is denied here with
   `SuspiciousActivity`, before any transaction is built.
5. **Fee-bump construction + signing** — pure, network-free
   (`src/fee-bump.ts`), fully unit-tested without touching a live network.
6. **Submission** — `SorobanRpc.Server.sendTransaction`.

Every step logs one structured JSON line via `src/logger.ts`
(`relay_attempt` / `relay_rejected` / `relay_eligible` / `relay_submitted`
/ `relay_failed`) for fraud-monitoring visibility.

## Setup

```bash
npm install
cp .env.example .env   # then fill in the values below
npm run dev             # ts-node, for local development
# or
npm run build && npm start
```

### Environment variables

| var | required | description |
|---|---|---|
| `SPONSOR_SECRET_KEY` | **yes** | Secret key (`S...`) of the funded Stellar account that pays every fee-bump's network fee. The service validates this at startup via `Keypair.fromSecret` and refuses to start if it's missing or malformed. |
| `CONTRACT_ID` | **yes** | Deployed Stellar Nebula Nomad contract ID (`C...`). |
| `SOROBAN_RPC_URL` | no (default `https://soroban-testnet.stellar.org`) | Soroban RPC endpoint. |
| `STELLAR_NETWORK_PASSPHRASE` | no (default Testnet) | Must match the network the contract is deployed on. |
| `PORT` | no (default `3001`) | HTTP port. |
| `FEE_BUMP_BASE_FEE` | no (default SDK `BASE_FEE`, 100 stroops) | Per-operation base fee for the fee-bump envelope. |
| `IP_RATE_LIMIT_MAX` / `IP_RATE_LIMIT_WINDOW_MS` | no (default 20 / 60000) | Per-source-IP sliding-window limit. |
| `ADDRESS_RATE_LIMIT_MAX` / `ADDRESS_RATE_LIMIT_WINDOW_MS` | no (default 5 / 60000) | Per-claimed-address sliding-window limit. |

The rate limiter is in-memory by default (`src/rate-limiter.ts`,
`InMemoryRateLimiter`) — fine for a single instance. `RedisRateLimiter` in
the same file implements the identical interface backed by a Redis sorted
set, for horizontal scaling across multiple relayer instances; wire it up
in `src/index.ts` in place of `InMemoryRateLimiter` if you're running more
than one.

## Known gap: contract wiring

`check_sponsorship_eligibility`, `get_sponsorship_pool`, and
`get_sponsorship_pool_size` exist in `src/gas_sponsor.rs` but are **not
yet exposed** as invokable entry points on `NebulaNomadContract` in
`src/lib.rs` — that file was out of scope for this change (see the task's
file boundaries). Until `src/lib.rs`'s `#[contractimpl]` block adds thin
wrappers for these functions (mirroring how `sponsor_first_scan` and the
other `gas_sponsor` functions are already re-exported), `checkEligibility`
calls from this service will fail against a deployed contract with a
"function not found" error. Required addition, roughly:

```rust
pub fn check_sponsorship_eligibility(env: Env, player: Address) -> Result<(), SponsorError> {
    gas_sponsor::check_sponsorship_eligibility(&env, &player)
}
```

## Testing

```bash
npm test
```

Everything except live network submission is covered by mocks that match
the real `@stellar/stellar-sdk` v11 type shapes (`SimulateTransactionSuccessResponse`,
`SimulateTransactionErrorResponse`, `SendTransactionResponse`) — see
`src/contract-client.test.ts`. Fee-bump construction
(`src/fee-bump.test.ts`) builds and signs real `Transaction`/
`FeeBumpTransaction` objects with throwaway `Keypair.random()` accounts, so
it verifies actual XDR structure, not a hand-rolled shape.

What is **not** covered, and requires a real funded sponsor account and a
live testnet contract deployment to verify end-to-end: actual transaction
finality (`getTransaction` polling), real fee-bump submission against
Soroban RPC, and the `check_sponsorship_eligibility` contract call once
`src/lib.rs` exposes it (see "Known gap" above).
