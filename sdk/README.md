# Stellar Nebula Nomad TypeScript SDK

Official TypeScript SDK for interacting with Stellar Nebula Nomad smart contracts.

## Installation

```bash
npm install @stellar-nebula/sdk
```

## Quick Start

```typescript
import {
  StellarNebulaClient,
  ShipType,
  ResourceType,
} from "@stellar-nebula/sdk";
import { Keypair, Networks } from "@stellar/stellar-sdk";

// Initialize the client
const client = new StellarNebulaClient({
  contractId: "YOUR_CONTRACT_ID",
  networkPassphrase: Networks.TESTNET,
  rpcUrl: "https://soroban-testnet.stellar.org",
});

// Create a keypair (or load existing)
const keypair = Keypair.random();

// Mint a new ship
const result = await client.mintShip(
  keypair,
  keypair.publicKey(),
  ShipType.Explorer,
);

if (result.success) {
  console.log("Ship minted! TX:", result.txHash);
  console.log("Ship ID:", result.result);
}
```

## Examples

### Scanning a Nebula

```typescript
const scanResult = await client.scanNebula(keypair, BigInt(1));

if (scanResult.success) {
  const layout = scanResult.result;
  console.log("Nebula layout:", layout);
  console.log("Rarity:", layout.rarity);
  console.log("Cells:", layout.cells);
}
```

### Harvesting Resources

```typescript
const harvestResult = await client.harvestResources(
  keypair,
  BigInt(1), // ship ID
  ResourceType.Minerals,
);

if (harvestResult.success) {
  console.log("Harvested amount:", harvestResult.result);
}
```

### Staking Resources

```typescript
const stakeResult = await client.stakeResources(
  keypair,
  ResourceType.Fuel,
  BigInt(1000),
  86400, // 1 day in seconds
);

if (stakeResult.success) {
  console.log("Resources staked successfully!");
}
```

### Querying Data

```typescript
// Get ship details
const ship = await client.getShip(BigInt(1));
console.log("Ship:", ship);

// Get resource balance
const balance = await client.getResourceBalance(
  keypair.publicKey(),
  ResourceType.Minerals,
);
console.log("Balance:", balance);
```

## API Reference

### Client Methods

#### `mintShip(caller, owner, shipType, options?)`

Mint a new ship NFT.

#### `scanNebula(caller, nebulaId, options?)`

Scan a nebula and generate its layout.

#### `harvestResources(caller, shipId, resourceType, options?)`

Harvest resources using a ship.

#### `stakeResources(caller, resourceType, amount, duration, options?)`

Stake resources for yield farming.

#### `claimYield(caller, stakeId, options?)`

Claim accumulated yield from staking.

#### `getShip(shipId)`

Query ship details by ID.

#### `getResourceBalance(address, resourceType)`

Query resource balance for an address.

## WalletConnect v2

Any client method that takes a `caller` accepts a `Keypair` (unchanged) or
anything implementing the `Signer` interface — including
`WalletConnectSigner`, which lets a user sign with a mobile/browser wallet
instead of a local secret key.

Requires a WalletConnect Cloud Project ID: get one at
https://cloud.walletconnect.com and set it as `WALLETCONNECT_PROJECT_ID`
(read automatically) or pass it explicitly via the `projectId` option.
Without it, `new WalletConnectSigner(...)` throws immediately — there is no
anonymous WalletConnect v2 mode.

```typescript
import {
  StellarNebulaClient,
  WalletConnectSigner,
  generatePairingQrCodeDataUrl,
  ShipType,
} from "@stellar-nebula/sdk";
import { Networks } from "@stellar/stellar-sdk";

const signer = new WalletConnectSigner({
  networkPassphrase: Networks.TESTNET, // resolves to the "stellar:testnet" CAIP-2 chain
  // projectId: "..." // or set WALLETCONNECT_PROJECT_ID in the environment
});

// 1. Begin pairing — `uri` is a `wc:...` link.
const pairing = await signer.connect();

// Desktop/web: render it as a QR code for the user's wallet to scan.
const qrDataUrl = await generatePairingQrCodeDataUrl(pairing.uri);

// Mobile: open it directly as a deep link instead (see mobile/src/wallet-connect-bridge.ts).

// 2. Wait for the wallet to approve the session.
const session = await pairing.approval();
console.log("Connected:", session.account);

// 3. Use the signer exactly like a Keypair.
const client = new StellarNebulaClient({
  contractId: "YOUR_CONTRACT_ID",
  networkPassphrase: Networks.TESTNET,
  rpcUrl: "https://soroban-testnet.stellar.org",
});
const result = await client.mintShip(signer, session.publicKey, ShipType.Explorer);

// Later, on a page reload / app restart: `new WalletConnectSigner(...)`
// automatically restores the session from storage if it's still active, so
// `signer.getPublicKey()` / `signer.signTransaction()` work without
// re-pairing. Disconnect explicitly when done:
await signer.disconnect();
```

By default sessions persist to `localStorage` when available (browser/web),
or in-memory otherwise. Pass a custom `storage` implementing
`WalletConnectStorage` (e.g. wrapping `@react-native-async-storage/async-storage`)
for other environments.

## Types

All TypeScript types are exported and available for import:

```typescript
import {
  Ship,
  NebulaLayout,
  ResourceBalance,
  ShipType,
  Rarity,
  ResourceType,
  CellType,
} from "@stellar-nebula/sdk";
```

## Error Handling

All transaction methods return a `TxResult` object:

```typescript
interface TxResult<T = any> {
  success: boolean;
  result?: T;
  error?: string;
  txHash?: string;
}
```

Always check the `success` field before accessing the result.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## License

MIT
