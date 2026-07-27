// expo-linking pulls in native-module glue that isn't meaningful (or safe to
// transpile) under plain Jest. Deep-link parsing and injection-script
// building are pure functions with no native dependency, so those are what
// get exercised here; the Linking-backed subscription wiring is exercised
// against an explicit mock rather than the real native module.
const mockGetInitialURL = jest.fn();
const mockAddEventListener = jest.fn();

jest.mock("expo-linking", () => ({
  getInitialURL: (...args: unknown[]) => mockGetInitialURL(...args),
  addEventListener: (...args: unknown[]) => mockAddEventListener(...args),
}));

import {
  buildWalletConnectUriInjection,
  isWalletConnectSessionMessage,
  parseWalletConnectDeepLink,
  parseWalletConnectSessionMessage,
  subscribeToWalletConnectDeepLinks,
} from "./wallet-connect-bridge";

const SAMPLE_PAIRING_URI =
  "wc:7f6e504bfad60b485450578e05678ed3e8e8c47bd1e0e024f5e5b9c2a7c5b1c@2?relay-protocol=irn&symKey=deadbeef";

beforeEach(() => {
  jest.clearAllMocks();
  mockGetInitialURL.mockResolvedValue(null);
  mockAddEventListener.mockReturnValue({ remove: jest.fn() });
});

describe("parseWalletConnectDeepLink", () => {
  it("returns a raw wc: URI unchanged", () => {
    expect(parseWalletConnectDeepLink(SAMPLE_PAIRING_URI)).toBe(
      SAMPLE_PAIRING_URI,
    );
  });

  it("extracts a wc: URI wrapped in a custom-scheme deep link's uri param", () => {
    const wrapped = `stellarnebulanomad://wc?uri=${encodeURIComponent(SAMPLE_PAIRING_URI)}`;
    expect(parseWalletConnectDeepLink(wrapped)).toBe(SAMPLE_PAIRING_URI);
  });

  it("extracts a wc: URI wrapped in a universal (https) link's uri param", () => {
    const wrapped = `https://stellar-nebula-nomad.app/wc?uri=${encodeURIComponent(SAMPLE_PAIRING_URI)}`;
    expect(parseWalletConnectDeepLink(wrapped)).toBe(SAMPLE_PAIRING_URI);
  });

  it("extracts the uri param when other query params precede it", () => {
    const wrapped = `myapp://wc?foo=bar&uri=${encodeURIComponent(SAMPLE_PAIRING_URI)}&baz=qux`;
    expect(parseWalletConnectDeepLink(wrapped)).toBe(SAMPLE_PAIRING_URI);
  });

  it("returns null for a link with no uri param", () => {
    expect(parseWalletConnectDeepLink("myapp://open?screen=home")).toBeNull();
  });

  it("returns null for a uri param that isn't a wc: URI", () => {
    expect(
      parseWalletConnectDeepLink("myapp://wc?uri=https%3A%2F%2Fexample.com"),
    ).toBeNull();
  });

  it("returns null for empty, null, or undefined input", () => {
    expect(parseWalletConnectDeepLink("")).toBeNull();
    expect(parseWalletConnectDeepLink(null)).toBeNull();
    expect(parseWalletConnectDeepLink(undefined)).toBeNull();
  });

  it("returns null for an unrelated deep link", () => {
    expect(
      parseWalletConnectDeepLink("stellarnebulanomad://game/level/3"),
    ).toBeNull();
  });
});

describe("buildWalletConnectUriInjection", () => {
  it("produces JS that safely embeds the URI as a JSON string literal", () => {
    const script = buildWalletConnectUriInjection(SAMPLE_PAIRING_URI);
    expect(script).toContain(JSON.stringify(SAMPLE_PAIRING_URI));
    expect(script).toContain("window.__stellarMobile.onWalletConnectURI");
  });

  it("escapes URIs containing quotes/backslashes so injected JS stays valid", () => {
    const tricky = 'wc:abc@2?relay-protocol=irn&symKey="; alert(1); //';
    const script = buildWalletConnectUriInjection(tricky);
    // JSON.stringify must be the only thing responsible for escaping — the
    // literal raw tricky string must not appear unescaped in the output.
    expect(script).not.toContain(`"${tricky}"`);
    expect(script).toContain(JSON.stringify(tricky));
  });
});

describe("wallet connect session message helpers", () => {
  it("isWalletConnectSessionMessage recognizes valid connected/disconnected payloads", () => {
    expect(
      isWalletConnectSessionMessage({
        type: "wallet_connect_session",
        status: "connected",
        account: "stellar:testnet:GABC",
      }),
    ).toBe(true);
    expect(
      isWalletConnectSessionMessage({
        type: "wallet_connect_session",
        status: "disconnected",
      }),
    ).toBe(true);
  });

  it("isWalletConnectSessionMessage rejects malformed or unrelated payloads", () => {
    expect(isWalletConnectSessionMessage(null)).toBe(false);
    expect(isWalletConnectSessionMessage({})).toBe(false);
    expect(isWalletConnectSessionMessage({ type: "something_else" })).toBe(
      false,
    );
    expect(
      isWalletConnectSessionMessage({
        type: "wallet_connect_session",
        status: "pending",
      }),
    ).toBe(false);
  });

  it("parseWalletConnectSessionMessage parses valid JSON session messages", () => {
    const raw = JSON.stringify({
      type: "wallet_connect_session",
      status: "connected",
      account: "stellar:pubnet:GXYZ",
      publicKey: "GXYZ",
    });
    expect(parseWalletConnectSessionMessage(raw)).toEqual({
      type: "wallet_connect_session",
      status: "connected",
      account: "stellar:pubnet:GXYZ",
      publicKey: "GXYZ",
    });
  });

  it("parseWalletConnectSessionMessage returns null for invalid JSON or unrelated messages", () => {
    expect(parseWalletConnectSessionMessage("not json")).toBeNull();
    expect(
      parseWalletConnectSessionMessage(JSON.stringify({ type: "other" })),
    ).toBeNull();
  });
});

describe("subscribeToWalletConnectDeepLinks", () => {
  it("checks getInitialURL on subscribe and forwards a pairing URI if found", async () => {
    mockGetInitialURL.mockResolvedValue(SAMPLE_PAIRING_URI);
    const onPairingUri = jest.fn();

    subscribeToWalletConnectDeepLinks({ onPairingUri });
    // getInitialURL resolution is async.
    await Promise.resolve();
    await Promise.resolve();

    expect(onPairingUri).toHaveBeenCalledWith(SAMPLE_PAIRING_URI);
  });

  it("does not forward when getInitialURL resolves a non-WalletConnect link", async () => {
    mockGetInitialURL.mockResolvedValue("myapp://game/level/3");
    const onPairingUri = jest.fn();

    subscribeToWalletConnectDeepLinks({ onPairingUri });
    await Promise.resolve();
    await Promise.resolve();

    expect(onPairingUri).not.toHaveBeenCalled();
  });

  it("registers a url event listener and forwards subsequent pairing links", () => {
    let registeredHandler: ((event: { url: string }) => void) | undefined;
    mockAddEventListener.mockImplementation(
      (_event: string, handler: (e: { url: string }) => void) => {
        registeredHandler = handler;
        return { remove: jest.fn() };
      },
    );
    const onPairingUri = jest.fn();

    subscribeToWalletConnectDeepLinks({ onPairingUri });
    expect(mockAddEventListener).toHaveBeenCalledWith(
      "url",
      expect.any(Function),
    );

    registeredHandler?.({ url: SAMPLE_PAIRING_URI });
    expect(onPairingUri).toHaveBeenCalledWith(SAMPLE_PAIRING_URI);
  });

  it("ignores non-WalletConnect urls delivered to the event listener", () => {
    let registeredHandler: ((event: { url: string }) => void) | undefined;
    mockAddEventListener.mockImplementation(
      (_event: string, handler: (e: { url: string }) => void) => {
        registeredHandler = handler;
        return { remove: jest.fn() };
      },
    );
    const onPairingUri = jest.fn();

    subscribeToWalletConnectDeepLinks({ onPairingUri });
    registeredHandler?.({ url: "myapp://home" });
    expect(onPairingUri).not.toHaveBeenCalled();
  });

  it("remove() delegates to the underlying subscription's remove()", () => {
    const removeMock = jest.fn();
    mockAddEventListener.mockReturnValue({ remove: removeMock });

    const subscription = subscribeToWalletConnectDeepLinks({
      onPairingUri: jest.fn(),
    });
    subscription.remove();

    expect(removeMock).toHaveBeenCalled();
  });
});
