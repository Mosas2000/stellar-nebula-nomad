import SignClient from "@walletconnect/sign-client";
import type { SessionTypes, SignClientTypes } from "@walletconnect/types";
import { getSdkError } from "@walletconnect/utils";
import {
  Signer,
  StellarWalletConnectChain,
  TransactionSignOptions,
  WalletConnectSessionInfo,
  WalletConnectStorage,
} from "./types";

/**
 * Stellar's WalletConnect namespace (CAIP-28: chainagnostic.org/CAIPs/caip-28).
 * Chain ids follow CAIP-2 ("stellar:pubnet" / "stellar:testnet"); methods
 * follow the convention used by Stellar wallets that support WalletConnect
 * v2 (LOBSTR, Stellar Wallets Kit, Reown/WalletConnect's own RPC reference).
 */
const STELLAR_NAMESPACE = "stellar";
export const STELLAR_METHOD_SIGN_XDR = "stellar_signXDR";
export const STELLAR_METHOD_SIGN_AND_SUBMIT_XDR = "stellar_signAndSubmitXDR";

const DEFAULT_STORAGE_KEY = "stellar-nebula:walletconnect:session";
const DEFAULT_RELAY_URL = "wss://relay.walletconnect.com";

/** The resolved instance type returned by `SignClient.init(...)`. */
type WcSignClient = Awaited<ReturnType<typeof SignClient.init>>;

export interface WalletConnectSignerOptions {
  /** The network this signer will produce signatures for. */
  networkPassphrase: string;
  /**
   * CAIP-2 chain id to request. Defaults to "stellar:testnet" if
   * `networkPassphrase` contains "Test", otherwise "stellar:pubnet".
   */
  chain?: StellarWalletConnectChain;
  /**
   * WalletConnect Cloud Project ID. Falls back to
   * `process.env.WALLETCONNECT_PROJECT_ID`. Required — WalletConnect v2 has
   * no anonymous/keyless mode.
   */
  projectId?: string;
  /** dApp metadata shown to the user in their wallet during pairing. */
  metadata?: SignClientTypes.Metadata;
  /** Override the default public WalletConnect relay. */
  relayUrl?: string;
  /** Where to persist session state. Defaults to localStorage if present, else in-memory. */
  storage?: WalletConnectStorage;
  /** Storage key used to persist session state. */
  storageKey?: string;
}

export interface WalletConnectPairing {
  /** `wc:...` pairing URI — render as a QR code or open as a mobile deep link. */
  uri: string;
  /** Resolves once the wallet approves the session. */
  approval: () => Promise<WalletConnectSessionInfo>;
}

function resolveChain(
  options: WalletConnectSignerOptions,
): StellarWalletConnectChain {
  if (options.chain) return options.chain;
  return options.networkPassphrase.toLowerCase().includes("test")
    ? "stellar:testnet"
    : "stellar:pubnet";
}

function inMemoryStorage(): WalletConnectStorage {
  const store = new Map<string, string>();
  return {
    getItem: (key) => (store.has(key) ? (store.get(key) as string) : null),
    setItem: (key, value) => {
      store.set(key, value);
    },
    removeItem: (key) => {
      store.delete(key);
    },
  };
}

function defaultStorage(): WalletConnectStorage {
  const globalScope = globalThis as unknown as {
    localStorage?: WalletConnectStorage;
  };
  if (globalScope.localStorage) {
    return globalScope.localStorage;
  }
  return inMemoryStorage();
}

/** Extracts the raw Stellar public key (G...) from a CAIP-10 account id. */
export function accountToPublicKey(account: string): string {
  const parts = account.split(":");
  return parts[parts.length - 1];
}

function isSessionExpiredError(error: unknown): boolean {
  const message =
    error && typeof (error as { message?: unknown }).message === "string"
      ? ((error as { message: string }).message as string)
      : "";
  return (
    message.includes("expired") ||
    message.includes("No matching key") ||
    message.includes("No matching topic")
  );
}

/**
 * A `Signer` backed by a live WalletConnect v2 session. Handles pairing
 * (QR / deep link), session persistence + restoration, and dispatching
 * `stellar_signXDR` requests to the connected wallet.
 */
export class WalletConnectSigner implements Signer {
  private readonly networkPassphrase: string;
  private readonly chain: StellarWalletConnectChain;
  private readonly projectId: string;
  private readonly metadata: SignClientTypes.Metadata;
  private readonly relayUrl: string;
  private readonly storage: WalletConnectStorage;
  private readonly storageKey: string;

  private client: WcSignClient | null = null;
  private session: WalletConnectSessionInfo | null = null;
  private initPromise: Promise<void> | null = null;

  constructor(options: WalletConnectSignerOptions) {
    const projectId = options.projectId ?? process.env.WALLETCONNECT_PROJECT_ID;
    if (!projectId) {
      throw new Error(
        "WalletConnect Project ID not configured. Obtain one at https://cloud.walletconnect.com and set WALLETCONNECT_PROJECT_ID.",
      );
    }

    this.projectId = projectId;
    this.networkPassphrase = options.networkPassphrase;
    this.chain = resolveChain(options);
    this.metadata = options.metadata ?? {
      name: "Stellar Nebula Nomad",
      description: "Space-exploration game on Stellar/Soroban",
      url: "https://stellar.org",
      icons: [],
    };
    this.relayUrl = options.relayUrl ?? DEFAULT_RELAY_URL;
    this.storage = options.storage ?? defaultStorage();
    this.storageKey = options.storageKey ?? DEFAULT_STORAGE_KEY;
  }

  /** Lazily initializes the underlying SignClient and attempts session restoration. */
  private async ensureClient(): Promise<WcSignClient> {
    if (!this.initPromise) {
      this.initPromise = (async () => {
        const client = await SignClient.init({
          projectId: this.projectId,
          relayUrl: this.relayUrl,
          metadata: this.metadata,
        });
        this.client = client;

        client.on("session_delete", () => {
          this.session = null;
          void this.persistSession();
        });
        client.on("session_expire", () => {
          this.session = null;
          void this.persistSession();
        });

        await this.restoreSession();
      })();
    }
    await this.initPromise;
    if (!this.client) {
      throw new Error("Failed to initialize the WalletConnect SignClient.");
    }
    return this.client;
  }

  /** Restores a persisted session if it still exists and hasn't expired. */
  private async restoreSession(): Promise<void> {
    const client = this.client;
    if (!client) return;

    const raw = await this.storage.getItem(this.storageKey);
    if (!raw) return;

    try {
      const stored: WalletConnectSessionInfo = JSON.parse(raw);
      const active = client.session
        .getAll()
        .find((entry: SessionTypes.Struct) => entry.topic === stored.topic);

      if (active && active.expiry * 1000 > Date.now()) {
        this.session = stored;
      } else {
        this.session = null;
        await this.storage.removeItem(this.storageKey);
      }
    } catch {
      await this.storage.removeItem(this.storageKey);
    }
  }

  private async persistSession(): Promise<void> {
    if (this.session) {
      await this.storage.setItem(this.storageKey, JSON.stringify(this.session));
    } else {
      await this.storage.removeItem(this.storageKey);
    }
  }

  private toSessionInfo(
    session: SessionTypes.Struct,
  ): WalletConnectSessionInfo {
    const accounts = session.namespaces[STELLAR_NAMESPACE]?.accounts ?? [];
    const account = accounts[0];
    if (!account) {
      throw new Error(
        "WalletConnect session approved without a Stellar account.",
      );
    }
    return {
      topic: session.topic,
      chainId: this.chain,
      account,
      publicKey: accountToPublicKey(account),
      expiry: session.expiry,
    };
  }

  /**
   * Begins a new pairing. Returns the `wc:` URI (render as a QR code for
   * desktop/web, or open directly as a deep link on mobile) and an
   * `approval()` function that resolves once the wallet approves.
   */
  async connect(): Promise<WalletConnectPairing> {
    const client = await this.ensureClient();

    const { uri, approval } = await client.connect({
      requiredNamespaces: {
        [STELLAR_NAMESPACE]: {
          chains: [this.chain],
          methods: [
            STELLAR_METHOD_SIGN_XDR,
            STELLAR_METHOD_SIGN_AND_SUBMIT_XDR,
          ],
          events: [],
        },
      },
    });

    if (!uri) {
      throw new Error("WalletConnect did not return a pairing URI.");
    }

    return {
      uri,
      approval: async () => {
        const approved = await approval();
        this.session = this.toSessionInfo(approved);
        await this.persistSession();
        return this.session;
      },
    };
  }

  /** True if a live (non-expired) session is currently restored/connected. */
  isConnected(): boolean {
    return this.session !== null && this.session.expiry * 1000 > Date.now();
  }

  /** Returns the current session info, or null if not connected. */
  getSession(): WalletConnectSessionInfo | null {
    return this.session;
  }

  async getPublicKey(): Promise<string> {
    await this.ensureClient();
    if (!this.session) {
      throw new Error(
        "No active WalletConnect session. Call connect() and await approval() first.",
      );
    }
    return this.session.publicKey;
  }

  async signTransaction(
    xdr: string,
    _opts: TransactionSignOptions,
  ): Promise<string> {
    const client = await this.ensureClient();
    if (!this.session) {
      throw new Error(
        "No active WalletConnect session. Call connect() and await approval() first.",
      );
    }

    try {
      const result = await client.request<{ signedXDR: string }>({
        topic: this.session.topic,
        chainId: this.chain,
        request: {
          method: STELLAR_METHOD_SIGN_XDR,
          params: { xdr },
        },
      });

      if (!result?.signedXDR) {
        throw new Error("WalletConnect wallet did not return a signed XDR.");
      }
      return result.signedXDR;
    } catch (error) {
      if (isSessionExpiredError(error)) {
        this.session = null;
        await this.persistSession();
      }
      throw error;
    }
  }

  /** Ends the active session, both on the relay and locally. */
  async disconnect(): Promise<void> {
    const client = await this.ensureClient();
    if (!this.session) return;

    const topic = this.session.topic;
    this.session = null;
    await this.persistSession();

    await client.disconnect({
      topic,
      reason: getSdkError("USER_DISCONNECTED"),
    });
  }
}
