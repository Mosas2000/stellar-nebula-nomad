import { Keypair } from "@stellar/stellar-sdk";
import { validateRelayRequest } from "./validation";

describe("validateRelayRequest", () => {
  const validAddress = Keypair.random().publicKey();

  it("accepts a well-formed request", () => {
    const result = validateRelayRequest({
      innerTransactionXdr: "AAAA...",
      playerAddress: validAddress,
    });
    expect(result.valid).toBe(true);
    expect(result.errors).toEqual([]);
  });

  it("rejects a non-object body", () => {
    expect(validateRelayRequest(null).valid).toBe(false);
    expect(validateRelayRequest("a string").valid).toBe(false);
    expect(validateRelayRequest(42).valid).toBe(false);
  });

  it("rejects a missing innerTransactionXdr", () => {
    const result = validateRelayRequest({ playerAddress: validAddress });
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("innerTransactionXdr"))).toBe(true);
  });

  it("rejects an empty innerTransactionXdr", () => {
    const result = validateRelayRequest({
      innerTransactionXdr: "   ",
      playerAddress: validAddress,
    });
    expect(result.valid).toBe(false);
  });

  it("rejects a missing playerAddress", () => {
    const result = validateRelayRequest({ innerTransactionXdr: "AAAA..." });
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("playerAddress"))).toBe(true);
  });

  it("rejects a malformed playerAddress", () => {
    const result = validateRelayRequest({
      innerTransactionXdr: "AAAA...",
      playerAddress: "not-a-real-stellar-address",
    });
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("must be a valid Stellar account"))).toBe(true);
  });

  it("rejects a contract address (C...) as playerAddress, only accounts allowed", () => {
    const result = validateRelayRequest({
      innerTransactionXdr: "AAAA...",
      playerAddress: "CCJZ5DGASBWQXR5MPFCJXMBI333XE5U3FSJTNQU7RIKE3P5GN2K2WYD5",
    });
    expect(result.valid).toBe(false);
  });

  it("collects multiple errors at once", () => {
    const result = validateRelayRequest({});
    expect(result.valid).toBe(false);
    expect(result.errors.length).toBe(2);
  });
});
