import express from "express";
import dotenv from "dotenv";
import { Networks } from "@stellar/stellar-sdk";
import { ContractClient } from "./contract-client";
import { InMemoryRateLimiter } from "./rate-limiter";
import { RelayerManager } from "./relayer-manager";
import { loadSponsorKeypair } from "./startup";
import { RelayRequest } from "./types";

dotenv.config();

// ─── Startup: fail fast on a missing/malformed sponsor account ───────────
// Never fabricate a placeholder secret and never let the service start in
// a silently-broken state — a gasless relayer with no funded sponsor
// account can't do its one job.
let sponsorKeypair;
try {
  sponsorKeypair = loadSponsorKeypair(process.env.SPONSOR_SECRET_KEY);
} catch (err) {
  // eslint-disable-next-line no-console
  console.error((err as Error).message);
  process.exit(1);
}

const RPC_URL = process.env.SOROBAN_RPC_URL || "https://soroban-testnet.stellar.org";
const CONTRACT_ID = process.env.CONTRACT_ID || "";
const NETWORK_PASSPHRASE = process.env.STELLAR_NETWORK_PASSPHRASE || Networks.TESTNET;
const PORT = process.env.PORT || 3001;

const IP_RATE_LIMIT_MAX = Number(process.env.IP_RATE_LIMIT_MAX || 20);
const IP_RATE_LIMIT_WINDOW_MS = Number(process.env.IP_RATE_LIMIT_WINDOW_MS || 60_000);
const ADDRESS_RATE_LIMIT_MAX = Number(process.env.ADDRESS_RATE_LIMIT_MAX || 5);
const ADDRESS_RATE_LIMIT_WINDOW_MS = Number(process.env.ADDRESS_RATE_LIMIT_WINDOW_MS || 60_000);

if (!CONTRACT_ID) {
  // eslint-disable-next-line no-console
  console.error(
    "CONTRACT_ID not configured. Set it to the deployed Stellar Nebula Nomad contract ID before starting the relayer.",
  );
  process.exit(1);
}

const contractClient = new ContractClient(
  RPC_URL,
  CONTRACT_ID,
  NETWORK_PASSPHRASE,
  sponsorKeypair.publicKey(),
);

const relayerManager = new RelayerManager({
  contractClient,
  sponsorKeypair,
  networkPassphrase: NETWORK_PASSPHRASE,
  baseFee: process.env.FEE_BUMP_BASE_FEE,
  ipRateLimiter: new InMemoryRateLimiter(IP_RATE_LIMIT_MAX, IP_RATE_LIMIT_WINDOW_MS),
  addressRateLimiter: new InMemoryRateLimiter(
    ADDRESS_RATE_LIMIT_MAX,
    ADDRESS_RATE_LIMIT_WINDOW_MS,
  ),
});

const app = express();
app.use(express.json());

/**
 * Relay a gasless transaction: validates the request, checks on-chain
 * sponsorship eligibility, then builds/signs/submits a fee-bump
 * transaction paying the fee on the player's behalf.
 *
 * See README.md for the full request/response contract.
 */
app.post("/relay", async (req, res) => {
  const sourceIp = req.ip || req.socket.remoteAddress || "unknown";
  const result = await relayerManager.relay(req.body as RelayRequest, sourceIp);

  switch (result.status) {
    case "submitted":
      res.status(200).json(result);
      return;
    case "rejected": {
      const statusCode =
        result.reason === "RATE_LIMITED"
          ? 429
          : result.reason === "SPONSORSHIP_INELIGIBLE"
            ? 403
            : 400;
      res.status(statusCode).json(result);
      return;
    }
    case "failed":
      res.status(502).json(result);
      return;
  }
});

app.get("/health", (_req, res) => {
  res.json({ status: "healthy", sponsor: sponsorKeypair.publicKey() });
});

app.listen(PORT, () => {
  // eslint-disable-next-line no-console
  console.log(
    `Gasless transaction relayer running on port ${PORT}, sponsor account ${sponsorKeypair.publicKey()}`,
  );
});

export { app, relayerManager };
