import type { WalletConnectStorage } from "./types";

// --- Mocks -----------------------------------------------------------------
// A live WalletConnect v2 session requires a real relay connection, which we
// deliberately never spin up in tests. Everything else (session lifecycle,
// persistence, request dispatch, error handling) is real logic and is
// exercised here against a mocked @walletconnect/sign-client.

type Handler = (...args: unknown[]) => void;

function createMockWcClient() {
  const handlers: Record<string, Handler> = {};
  return {
    on: jest.fn((event: string, handler: Handler) => {
      handlers[event] = handler;
    }),
    connect: jest.fn(),
    request: jest.fn(),
    disconnect: jest.fn().mockResolvedValue(undefined),
    session: { getAll: jest.fn(() => [] as unknown[]) },
    __emit: (event: string, ...args: unknown[]) => handlers[event]?.(...args),
  };
}

const mockClient = createMockWcClient();

jest.mock("@walletconnect/sign-client", () => ({
  __esModule: true,
  default: {
    init: jest.fn(),
  },
}));

jest.mock("@walletconnect/utils", () => ({
  getSdkError: jest.fn((code: string) => ({ code: 6000, message: code })),
}));

import SignClientDefault from "@walletconnect/sign-client";
import {
  WalletConnectSigner,
  accountToPublicKey,
  STELLAR_METHOD_SIGN_XDR,
} from "./wallet-connect";

const SignClientMock = SignClientDefault as unknown as {
  init: jest.Mock;
};

function createTestStorage(): WalletConnectStorage & {
  data: Map<string, string>;
} {
  const data = new Map<string, string>();
  return {
    data,
    getItem: jest.fn((key: string) => (data.has(key) ? data.get(key)! : null)),
    setItem: jest.fn((key: string, value: string) => {
      data.set(key, value);
    }),
    removeItem: jest.fn((key: string) => {
      data.delete(key);
    }),
  } as unknown as WalletConnectStorage & { data: Map<string, string> };
}

const TEST_PUBLIC_KEY =
  "GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRST";
const TEST_ACCOUNT = `stellar:testnet:${TEST_PUBLIC_KEY}`;

function futureExpiry(seconds = 3600): number {
  return Math.floor(Date.now() / 1000) + seconds;
}

function pastExpiry(): number {
  return Math.floor(Date.now() / 1000) - 3600;
}

beforeEach(() => {
  jest.clearAllMocks();
  mockClient.session.getAll.mockReturnValue([]);
  SignClientMock.init.mockResolvedValue(mockClient);
  delete process.env.WALLETCONNECT_PROJECT_ID;
});

describe("accountToPublicKey", () => {
  it("extracts the raw public key from a CAIP-10 account id", () => {
    expect(accountToPublicKey(TEST_ACCOUNT)).toBe(TEST_PUBLIC_KEY);
  });
});

describe("WalletConnectSigner construction", () => {
  it("throws a clear error when no Project ID is configured (option or env)", () => {
    expect(
      () => new WalletConnectSigner({ networkPassphrase: "Test SDF Network" }),
    ).toThrow(
      "WalletConnect Project ID not configured. Obtain one at https://cloud.walletconnect.com and set WALLETCONNECT_PROJECT_ID.",
    );
  });

  it("accepts a Project ID passed directly as an option", () => {
    expect(
      () =>
        new WalletConnectSigner({
          networkPassphrase: "Test SDF Network",
          projectId: "explicit-project-id",
          storage: createTestStorage(),
        }),
    ).not.toThrow();
  });

  it("falls back to WALLETCONNECT_PROJECT_ID from the environment", () => {
    process.env.WALLETCONNECT_PROJECT_ID = "env-project-id";
    expect(
      () =>
        new WalletConnectSigner({
          networkPassphrase: "Test SDF Network",
          storage: createTestStorage(),
        }),
    ).not.toThrow();
  });

  it("does not initialize the SignClient until first use (lazy init)", () => {
    new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage: createTestStorage(),
    });
    expect(SignClientMock.init).not.toHaveBeenCalled();
  });
});

describe("WalletConnectSigner.connect / pairing + session persistence", () => {
  it("returns a pairing URI and, on approval, persists session info", async () => {
    const storage = createTestStorage();
    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });

    const expiry = futureExpiry();
    mockClient.connect.mockResolvedValue({
      uri: "wc:abcd1234@2?relay-protocol=irn&symKey=deadbeef",
      approval: jest.fn().mockResolvedValue({
        topic: "topic-1",
        expiry,
        namespaces: {
          stellar: { accounts: [TEST_ACCOUNT] },
        },
      }),
    });

    const pairing = await signer.connect();
    expect(pairing.uri).toBe("wc:abcd1234@2?relay-protocol=irn&symKey=deadbeef");

    expect(mockClient.connect).toHaveBeenCalledWith({
      requiredNamespaces: {
        stellar: {
          chains: ["stellar:testnet"],
          methods: [
            "stellar_signXDR",
            "stellar_signAndSubmitXDR",
          ],
          events: [],
        },
      },
    });

    const session = await pairing.approval();
    expect(session).toEqual({
      topic: "topic-1",
      chainId: "stellar:testnet",
      account: TEST_ACCOUNT,
      publicKey: TEST_PUBLIC_KEY,
      expiry,
    });

    expect(signer.isConnected()).toBe(true);
    expect(storage.setItem).toHaveBeenCalledWith(
      "stellar-nebula:walletconnect:session",
      JSON.stringify(session),
    );
  });

  it("defaults to the pubnet chain when the network passphrase is not testnet", async () => {
    const storage = createTestStorage();
    const signer = new WalletConnectSigner({
      networkPassphrase: "Public Global Stellar Network ; September 2015",
      projectId: "p",
      storage,
    });

    mockClient.connect.mockResolvedValue({
      uri: "wc:xyz@2",
      approval: jest.fn(),
    });

    await signer.connect();
    expect(mockClient.connect).toHaveBeenCalledWith(
      expect.objectContaining({
        requiredNamespaces: expect.objectContaining({
          stellar: expect.objectContaining({ chains: ["stellar:pubnet"] }),
        }),
      }),
    );
  });

  it("throws if the wallet approves without a Stellar account", async () => {
    const storage = createTestStorage();
    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });

    mockClient.connect.mockResolvedValue({
      uri: "wc:abcd@2",
      approval: jest.fn().mockResolvedValue({
        topic: "topic-x",
        expiry: futureExpiry(),
        namespaces: { stellar: { accounts: [] } },
      }),
    });

    const pairing = await signer.connect();
    await expect(pairing.approval()).rejects.toThrow(
      /approved without a Stellar account/,
    );
  });
});

describe("WalletConnectSigner session restoration", () => {
  it("restores a persisted, still-active session without re-pairing", async () => {
    const storage = createTestStorage();
    const expiry = futureExpiry();
    storage.data.set(
      "stellar-nebula:walletconnect:session",
      JSON.stringify({
        topic: "restored-topic",
        chainId: "stellar:testnet",
        account: TEST_ACCOUNT,
        publicKey: TEST_PUBLIC_KEY,
        expiry,
      }),
    );
    mockClient.session.getAll.mockReturnValue([
      { topic: "restored-topic", expiry },
    ]);

    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });

    await expect(signer.getPublicKey()).resolves.toBe(TEST_PUBLIC_KEY);
    expect(signer.isConnected()).toBe(true);
  });

  it("discards a persisted session whose topic is no longer active on the client", async () => {
    const storage = createTestStorage();
    storage.data.set(
      "stellar-nebula:walletconnect:session",
      JSON.stringify({
        topic: "stale-topic",
        chainId: "stellar:testnet",
        account: TEST_ACCOUNT,
        publicKey: TEST_PUBLIC_KEY,
        expiry: futureExpiry(),
      }),
    );
    mockClient.session.getAll.mockReturnValue([]); // relay no longer knows this topic

    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });

    await expect(signer.getPublicKey()).rejects.toThrow(
      /No active WalletConnect session/,
    );
    expect(storage.removeItem).toHaveBeenCalledWith(
      "stellar-nebula:walletconnect:session",
    );
  });

  it("discards a persisted session that has expired", async () => {
    const storage = createTestStorage();
    storage.data.set(
      "stellar-nebula:walletconnect:session",
      JSON.stringify({
        topic: "expired-topic",
        chainId: "stellar:testnet",
        account: TEST_ACCOUNT,
        publicKey: TEST_PUBLIC_KEY,
        expiry: pastExpiry(),
      }),
    );
    mockClient.session.getAll.mockReturnValue([
      { topic: "expired-topic", expiry: pastExpiry() },
    ]);

    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });

    await expect(signer.getPublicKey()).rejects.toThrow(
      /No active WalletConnect session/,
    );
  });

  it("discards unparseable persisted session data", async () => {
    const storage = createTestStorage();
    storage.data.set("stellar-nebula:walletconnect:session", "{not-json");

    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });

    await expect(signer.getPublicKey()).rejects.toThrow(
      /No active WalletConnect session/,
    );
    expect(storage.removeItem).toHaveBeenCalled();
  });
});

describe("WalletConnectSigner.signTransaction", () => {
  async function connectedSigner(storage: WalletConnectStorage) {
    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });
    mockClient.connect.mockResolvedValue({
      uri: "wc:abcd@2",
      approval: jest.fn().mockResolvedValue({
        topic: "topic-sign",
        expiry: futureExpiry(),
        namespaces: { stellar: { accounts: [TEST_ACCOUNT] } },
      }),
    });
    const pairing = await signer.connect();
    await pairing.approval();
    return signer;
  }

  it("throws when there is no active session", async () => {
    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage: createTestStorage(),
    });
    await expect(
      signer.signTransaction("AAAA", { networkPassphrase: "Test SDF Network" }),
    ).rejects.toThrow(/No active WalletConnect session/);
  });

  it("dispatches a stellar_signXDR request over the active session and returns the signed XDR", async () => {
    const storage = createTestStorage();
    const signer = await connectedSigner(storage);

    mockClient.request.mockResolvedValue({ signedXDR: "SIGNED_XDR_BASE64" });

    const result = await signer.signTransaction("UNSIGNED_XDR_BASE64", {
      networkPassphrase: "Test SDF Network",
    });

    expect(result).toBe("SIGNED_XDR_BASE64");
    expect(mockClient.request).toHaveBeenCalledWith({
      topic: "topic-sign",
      chainId: "stellar:testnet",
      request: {
        method: STELLAR_METHOD_SIGN_XDR,
        params: { xdr: "UNSIGNED_XDR_BASE64" },
      },
    });
  });

  it("throws if the wallet response omits signedXDR", async () => {
    const storage = createTestStorage();
    const signer = await connectedSigner(storage);
    mockClient.request.mockResolvedValue({});

    await expect(
      signer.signTransaction("XDR", { networkPassphrase: "Test SDF Network" }),
    ).rejects.toThrow(/did not return a signed XDR/);
  });

  it("clears the session when the relay reports it expired mid-request", async () => {
    const storage = createTestStorage();
    const signer = await connectedSigner(storage);
    mockClient.request.mockRejectedValue(new Error("No matching key. session topic doesn't exist"));

    await expect(
      signer.signTransaction("XDR", { networkPassphrase: "Test SDF Network" }),
    ).rejects.toThrow(/No matching key/);

    expect(signer.getSession()).toBeNull();
    expect(storage.removeItem).toHaveBeenCalledWith(
      "stellar-nebula:walletconnect:session",
    );
  });
});

describe("WalletConnectSigner session lifecycle events", () => {
  it("clears the session on session_delete", async () => {
    const storage = createTestStorage();
    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });
    mockClient.connect.mockResolvedValue({
      uri: "wc:abcd@2",
      approval: jest.fn().mockResolvedValue({
        topic: "topic-evt",
        expiry: futureExpiry(),
        namespaces: { stellar: { accounts: [TEST_ACCOUNT] } },
      }),
    });
    const pairing = await signer.connect();
    await pairing.approval();
    expect(signer.isConnected()).toBe(true);

    mockClient.__emit("session_delete");
    // Event handlers persist asynchronously; flush microtasks.
    await Promise.resolve();
    await Promise.resolve();

    expect(signer.getSession()).toBeNull();
  });
});

describe("WalletConnectSigner.disconnect", () => {
  it("disconnects the active session and clears persisted state", async () => {
    const storage = createTestStorage();
    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage,
    });
    mockClient.connect.mockResolvedValue({
      uri: "wc:abcd@2",
      approval: jest.fn().mockResolvedValue({
        topic: "topic-disc",
        expiry: futureExpiry(),
        namespaces: { stellar: { accounts: [TEST_ACCOUNT] } },
      }),
    });
    const pairing = await signer.connect();
    await pairing.approval();

    await signer.disconnect();

    expect(mockClient.disconnect).toHaveBeenCalledWith({
      topic: "topic-disc",
      reason: { code: 6000, message: "USER_DISCONNECTED" },
    });
    expect(signer.getSession()).toBeNull();
    expect(storage.removeItem).toHaveBeenCalledWith(
      "stellar-nebula:walletconnect:session",
    );
  });

  it("is a no-op when there is no active session", async () => {
    const signer = new WalletConnectSigner({
      networkPassphrase: "Test SDF Network",
      projectId: "p",
      storage: createTestStorage(),
    });
    await expect(signer.disconnect()).resolves.toBeUndefined();
    expect(mockClient.disconnect).not.toHaveBeenCalled();
  });
});
