import { Account, Keypair } from "@stellar/stellar-sdk";
import { ContractClient, parseContractErrorCode } from "./contract-client";
import { SponsorErrorCode } from "./types";

const mockGetAccount = jest.fn();
const mockSimulateTransaction = jest.fn();
const mockSendTransaction = jest.fn();

jest.mock("@stellar/stellar-sdk", () => {
  const actual = jest.requireActual("@stellar/stellar-sdk");
  return {
    ...actual,
    SorobanRpc: {
      ...actual.SorobanRpc,
      Server: jest.fn().mockImplementation(() => ({
        getAccount: mockGetAccount,
        simulateTransaction: mockSimulateTransaction,
        sendTransaction: mockSendTransaction,
      })),
    },
  };
});

const CONTRACT_ID = "CCJZ5DGASBWQXR5MPFCJXMBI333XE5U3FSJTNQU7RIKE3P5GN2K2WYD5";
const NETWORK_PASSPHRASE = "Test SDF Network ; September 2015";

describe("parseContractErrorCode", () => {
  it("extracts a known contract error code from a simulation error string", () => {
    const err = "HostError: Error(Contract, #10)\n\nEvent log (newest first):\n  ...";
    expect(parseContractErrorCode(err)).toBe(SponsorErrorCode.SuspiciousActivity);
  });

  it("extracts error code 1 (AlreadySponsored)", () => {
    expect(parseContractErrorCode("Error(Contract, #1)")).toBe(
      SponsorErrorCode.AlreadySponsored,
    );
  });

  it("returns undefined for an unrecognized code", () => {
    expect(parseContractErrorCode("Error(Contract, #9999)")).toBeUndefined();
  });

  it("returns undefined for a non-contract error (e.g. network failure)", () => {
    expect(parseContractErrorCode("connect ECONNREFUSED 127.0.0.1:8000")).toBeUndefined();
  });
});

describe("ContractClient", () => {
  const sponsor = Keypair.random();
  const player = Keypair.random();

  let client: ContractClient;

  beforeEach(() => {
    mockGetAccount.mockReset();
    mockSimulateTransaction.mockReset();
    mockSendTransaction.mockReset();
    client = new ContractClient(
      "https://rpc.example.invalid",
      CONTRACT_ID,
      NETWORK_PASSPHRASE,
      sponsor.publicKey(),
    );
  });

  it("returns eligible=true when simulation succeeds", async () => {
    mockGetAccount.mockResolvedValue(new Account(sponsor.publicKey(), "1"));
    mockSimulateTransaction.mockResolvedValue({
      id: "1",
      latestLedger: 100,
      events: [],
      _parsed: true,
      transactionData: {},
      minResourceFee: "100",
      cost: { cpuInsns: "1", memBytes: "1" },
      result: undefined,
    });

    const result = await client.checkEligibility(player.publicKey());
    expect(result.eligible).toBe(true);
    expect(mockGetAccount).toHaveBeenCalledWith(sponsor.publicKey());
  });

  it("returns eligible=false with the parsed code when simulation reports a contract error", async () => {
    mockGetAccount.mockResolvedValue(new Account(sponsor.publicKey(), "1"));
    mockSimulateTransaction.mockResolvedValue({
      id: "1",
      latestLedger: 100,
      events: [],
      _parsed: true,
      error: "HostError: Error(Contract, #10)\n\nEvent log (newest first):\n  ...",
    });

    const result = await client.checkEligibility(player.publicKey());
    expect(result.eligible).toBe(false);
    if (!result.eligible) {
      expect(result.code).toBe(SponsorErrorCode.SuspiciousActivity);
      expect(result.message).toMatch(/bot detection/);
    }
  });

  it("returns eligible=false with the raw message when the error is unrecognized", async () => {
    mockGetAccount.mockResolvedValue(new Account(sponsor.publicKey(), "1"));
    mockSimulateTransaction.mockResolvedValue({
      id: "1",
      latestLedger: 100,
      events: [],
      _parsed: true,
      error: "some unrelated RPC failure",
    });

    const result = await client.checkEligibility(player.publicKey());
    expect(result.eligible).toBe(false);
    if (!result.eligible) {
      expect(result.code).toBeUndefined();
      expect(result.message).toBe("some unrelated RPC failure");
    }
  });

  it("submits a fee-bump transaction and returns hash/status on success", async () => {
    mockSendTransaction.mockResolvedValue({
      status: "PENDING",
      hash: "abc123",
      latestLedger: 100,
      latestLedgerCloseTime: 0,
    });

    const fakeFeeBumpTx = { toXDR: () => "AAAA" } as unknown as Parameters<
      ContractClient["submit"]
    >[0];
    const result = await client.submit(fakeFeeBumpTx);

    expect(result).toEqual({ hash: "abc123", status: "PENDING" });
  });

  it("throws when the network rejects the fee-bump submission", async () => {
    mockSendTransaction.mockResolvedValue({
      status: "ERROR",
      hash: "abc123",
      latestLedger: 100,
      latestLedgerCloseTime: 0,
    });

    const fakeFeeBumpTx = { toXDR: () => "AAAA" } as unknown as Parameters<
      ContractClient["submit"]
    >[0];

    await expect(client.submit(fakeFeeBumpTx)).rejects.toThrow(/rejected/);
  });
});
