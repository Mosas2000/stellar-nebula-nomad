import { Keypair } from "@stellar/stellar-sdk";
import {
  loadSponsorKeypair,
  MISSING_SPONSOR_SECRET_MESSAGE,
  MissingSponsorSecretError,
} from "./startup";

describe("loadSponsorKeypair", () => {
  it("throws MissingSponsorSecretError with the exact required message when unset", () => {
    expect(() => loadSponsorKeypair(undefined)).toThrow(MissingSponsorSecretError);
    expect(() => loadSponsorKeypair(undefined)).toThrow(MISSING_SPONSOR_SECRET_MESSAGE);
  });

  it("throws MissingSponsorSecretError when the env var is empty/whitespace", () => {
    expect(() => loadSponsorKeypair("")).toThrow(MissingSponsorSecretError);
    expect(() => loadSponsorKeypair("   ")).toThrow(MissingSponsorSecretError);
  });

  it("lets Keypair.fromSecret throw naturally on a malformed secret", () => {
    expect(() => loadSponsorKeypair("not-a-real-secret-key")).toThrow();
    expect(() => loadSponsorKeypair("Sxxxx")).toThrow();
    // A well-formed-looking prefix with a broken checksum must still fail.
    expect(() =>
      loadSponsorKeypair("SBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
    ).toThrow();
  });

  it("returns a usable Keypair for a valid secret", () => {
    const generated = Keypair.random();
    const loaded = loadSponsorKeypair(generated.secret());
    expect(loaded.publicKey()).toBe(generated.publicKey());
    expect(loaded.canSign()).toBe(true);
  });
});
