import { Account, Keypair, Networks, Operation, TransactionBuilder } from "@stellar/stellar-sdk";
import { ContractClient } from "./contract-client";
import { RelayerManager } from "./relayer-manager";
import { RateLimiter, RelayRequest, SponsorErrorCode } from "./types";

const NETWORK_PASSPHRASE = Networks.TESTNET;

function buildSignedInnerTxXdr(playerKeypair: Keypair, sequence = "100"): string {
  const account = new Account(playerKeypair.publicKey(), sequence);
  const tx = new TransactionBuilder(account, {
    fee: "0",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(Operation.manageData({ name: "sponsor-me", value: "1" }))
    .setTimeout(30)
    .build();
  tx.sign(playerKeypair);
  return tx.toXDR();
}

function allowAllLimiter(): RateLimiter {
  return { checkAndRecord: jest.fn().mockResolvedValue({ allowed: true, remaining: 10 }) };
}

function denyLimiter(): RateLimiter {
  return {
    checkAndRecord: jest
      .fn()
      .mockResolvedValue({ allowed: false, remaining: 0, retryAfterMs: 5000 }),
  };
}

function mockContractClient(overrides?: Partial<ContractClient>): ContractClient {
  return {
    checkEligibility: jest.fn().mockResolvedValue({ eligible: true }),
    submit: jest.fn().mockResolvedValue({ hash: "deadbeef", status: "PENDING" }),
    ...overrides,
  } as unknown as ContractClient;
}

describe("RelayerManager.relay", () => {
  const sponsor = Keypair.random();

  it("rejects a malformed request before touching rate limits or the network", async () => {
    const contractClient = mockContractClient();
    const ipLimiter = allowAllLimiter();
    const manager = new RelayerManager({
      contractClient,
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      ipRateLimiter: ipLimiter,
      addressRateLimiter: allowAllLimiter(),
    });

    const result = await manager.relay({} as RelayRequest, "1.2.3.4");

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.reason).toBe("INVALID_REQUEST");
    }
    expect(ipLimiter.checkAndRecord).not.toHaveBeenCalled();
    expect(contractClient.checkEligibility).not.toHaveBeenCalled();
  });

  it("rejects when the per-IP rate limit is exceeded", async () => {
    const player = Keypair.random();
    const manager = new RelayerManager({
      contractClient: mockContractClient(),
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      ipRateLimiter: denyLimiter(),
      addressRateLimiter: allowAllLimiter(),
    });

    const result = await manager.relay(
      { innerTransactionXdr: buildSignedInnerTxXdr(player), playerAddress: player.publicKey() },
      "1.2.3.4",
    );

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.reason).toBe("RATE_LIMITED");
    }
  });

  it("rejects when the per-address rate limit is exceeded", async () => {
    const player = Keypair.random();
    const contractClient = mockContractClient();
    const manager = new RelayerManager({
      contractClient,
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      ipRateLimiter: allowAllLimiter(),
      addressRateLimiter: denyLimiter(),
    });

    const result = await manager.relay(
      { innerTransactionXdr: buildSignedInnerTxXdr(player), playerAddress: player.publicKey() },
      "1.2.3.4",
    );

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.reason).toBe("RATE_LIMITED");
    }
    expect(contractClient.checkEligibility).not.toHaveBeenCalled();
  });

  it("rejects an undecodable inner transaction", async () => {
    const player = Keypair.random();
    const manager = new RelayerManager({
      contractClient: mockContractClient(),
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      ipRateLimiter: allowAllLimiter(),
      addressRateLimiter: allowAllLimiter(),
    });

    const result = await manager.relay(
      { innerTransactionXdr: "not-valid-xdr", playerAddress: player.publicKey() },
      "1.2.3.4",
    );

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.reason).toBe("INVALID_TRANSACTION_ENVELOPE");
    }
  });

  it("rejects when the inner transaction's source doesn't match the claimed playerAddress", async () => {
    const player = Keypair.random();
    const impostor = Keypair.random();
    const manager = new RelayerManager({
      contractClient: mockContractClient(),
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      ipRateLimiter: allowAllLimiter(),
      addressRateLimiter: allowAllLimiter(),
    });

    const result = await manager.relay(
      {
        innerTransactionXdr: buildSignedInnerTxXdr(player),
        playerAddress: impostor.publicKey(),
      },
      "1.2.3.4",
    );

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.reason).toBe("SOURCE_ACCOUNT_MISMATCH");
    }
  });

  it("rejects when the on-chain eligibility check denies the request (e.g. bot-flagged)", async () => {
    const player = Keypair.random();
    const contractClient = mockContractClient({
      checkEligibility: jest.fn().mockResolvedValue({
        eligible: false,
        code: SponsorErrorCode.SuspiciousActivity,
        message: "player is flagged as high-risk by bot detection (CAPTCHA required)",
      }),
    });
    const manager = new RelayerManager({
      contractClient,
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      ipRateLimiter: allowAllLimiter(),
      addressRateLimiter: allowAllLimiter(),
    });

    const result = await manager.relay(
      { innerTransactionXdr: buildSignedInnerTxXdr(player), playerAddress: player.publicKey() },
      "1.2.3.4",
    );

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.reason).toBe("SPONSORSHIP_INELIGIBLE");
      expect(result.detail).toMatch(/bot detection/);
    }
    expect(contractClient.submit).not.toHaveBeenCalled();
  });

  it("builds, signs, and submits a fee-bump transaction on the happy path", async () => {
    const player = Keypair.random();
    const contractClient = mockContractClient();
    const manager = new RelayerManager({
      contractClient,
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      ipRateLimiter: allowAllLimiter(),
      addressRateLimiter: allowAllLimiter(),
    });

    const result = await manager.relay(
      { innerTransactionXdr: buildSignedInnerTxXdr(player), playerAddress: player.publicKey() },
      "1.2.3.4",
    );

    expect(result.status).toBe("submitted");
    if (result.status === "submitted") {
      expect(result.hash).toBe("deadbeef");
      expect(result.feeBumpHash).toBeTruthy();
    }
    expect(contractClient.submit).toHaveBeenCalledTimes(1);
  });

  it("returns a failed result when submission throws", async () => {
    const player = Keypair.random();
    const contractClient = mockContractClient({
      submit: jest.fn().mockRejectedValue(new Error("network rejected the fee-bump transaction")),
    });
    const manager = new RelayerManager({
      contractClient,
      sponsorKeypair: sponsor,
      networkPassphrase: NETWORK_PASSPHRASE,
      ipRateLimiter: allowAllLimiter(),
      addressRateLimiter: allowAllLimiter(),
    });

    const result = await manager.relay(
      { innerTransactionXdr: buildSignedInnerTxXdr(player), playerAddress: player.publicKey() },
      "1.2.3.4",
    );

    expect(result.status).toBe("failed");
    if (result.status === "failed") {
      expect(result.reason).toMatch(/rejected/);
    }
  });
});
