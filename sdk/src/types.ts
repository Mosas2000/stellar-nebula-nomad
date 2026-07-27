import { Account, Operation, Transaction, xdr } from "@stellar/stellar-sdk";

// Core types
export interface ContractConfig {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

export interface TransactionOptions {
  fee?: string;
  timeout?: number;
}

// Ship types
export interface Ship {
  id: bigint;
  owner: string;
  shipType: ShipType;
  rarity: Rarity;
  stats: ShipStats;
}

export enum ShipType {
  Explorer = 0,
  Fighter = 1,
  Trader = 2,
  Miner = 3,
}

export enum Rarity {
  Common = 0,
  Uncommon = 1,
  Rare = 2,
  Epic = 3,
  Legendary = 4,
}

export interface ShipStats {
  speed: number;
  cargo: number;
  weapons: number;
  shields: number;
}

// Nebula types
export interface NebulaLayout {
  seed: bigint;
  width: number;
  height: number;
  cells: Cell[];
  rarity: Rarity;
  timestamp: bigint;
}

export interface Cell {
  x: number;
  y: number;
  cellType: CellType;
  energy: number;
}

export enum CellType {
  Empty = 0,
  Resource = 1,
  Hazard = 2,
  Portal = 3,
}

// Resource types
export interface ResourceBalance {
  resourceType: ResourceType;
  amount: bigint;
}

export enum ResourceType {
  Fuel = 0,
  Minerals = 1,
  Alloys = 2,
  Crystals = 3,
}

// Event types
export interface ContractEvent {
  type: string;
  data: Record<string, any>;
  ledger: number;
  txHash: string;
}

// Transaction result
export interface TxResult<T = any> {
  success: boolean;
  result?: T;
  error?: string;
  txHash?: string;
}

// ---------------------------------------------------------------------------
// Signer abstraction
//
// StellarNebulaClient historically signed transactions with a raw Keypair.
// The Signer interface lets it work with any signing backend (a local
// Keypair, a WalletConnect session, a hardware wallet, etc.) without the
// client needing to know which one it's talking to.
// ---------------------------------------------------------------------------

export interface TransactionSignOptions {
  networkPassphrase: string;
}

export interface Signer {
  /** Returns the Stellar public key (G...) this signer will sign for. */
  getPublicKey(): Promise<string>;
  /** Signs a transaction envelope XDR (base64) and returns the signed XDR. */
  signTransaction(
    xdr: string,
    opts: TransactionSignOptions,
  ): Promise<string>;
}

// ---------------------------------------------------------------------------
// WalletConnect v2 types
// ---------------------------------------------------------------------------

/** CAIP-2 chain identifiers for the Stellar namespace (CAIP-28). */
export type StellarWalletConnectChain = "stellar:pubnet" | "stellar:testnet";

/** Minimal persisted state needed to restore a WalletConnect session without re-pairing. */
export interface WalletConnectSessionInfo {
  topic: string;
  chainId: StellarWalletConnectChain;
  /** Full CAIP-10 account id, e.g. "stellar:pubnet:GABC...". */
  account: string;
  /** Raw Stellar public key extracted from `account`. */
  publicKey: string;
  /** Unix seconds when the session expires. */
  expiry: number;
}

/**
 * Storage abstraction used to persist WalletConnect session state across
 * page reloads / app restarts. Implementations may be sync or async
 * (localStorage is sync, AsyncStorage is async — both are supported).
 */
export interface WalletConnectStorage {
  getItem(key: string): Promise<string | null> | string | null;
  setItem(key: string, value: string): Promise<void> | void;
  removeItem(key: string): Promise<void> | void;
}
