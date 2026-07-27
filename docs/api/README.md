# Stellar Nebula Nomad — API Reference

The contract exposes all operations as Soroban invocations on the Stellar network. This guide covers authentication, error handling, and usage examples for each endpoint group. The full machine-readable spec lives in [`openapi.yaml`](./openapi.yaml).

## Authentication

Every mutating call requires a valid Stellar keypair. The `caller` parameter must match the transaction source account — the SDK enforces `caller.require_auth()` on-chain. Read-only queries (e.g. `get_profile`, `get_ship`) do not require auth.

## Invoking the contract

All calls go through the Stellar RPC endpoint using `stellar contract invoke` or one of the SDK helpers.

```sh
# Generic pattern
stellar contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  --source-account $SECRET_KEY \
  -- <function_name> [--param value …]
```

Set these once in your shell:

```sh
export CONTRACT_ID=C...          # deployed contract address
export SOURCE=S...               # your Stellar secret key
export NETWORK=testnet           # or mainnet
```

## Error codes

| Code | Name | Description |
|------|------|-------------|
| 1 | AlreadyExists | Resource already exists (profile, bond, etc.) |
| 2 | NotFound | Requested resource was not found |
| 3 | Unauthorized | Caller is not authorized for this action |
| 4 | BatchTooLarge | Batch size exceeds the per-call limit |
| 5 | IncompatibleVersion | Target version is not supported |
| 6 | AlreadyMigrated | Migration already completed for this version pair |
| 7 | MigrationInProgress | Migration is still in progress |
| 8 | AlreadyUnlocked | Achievement has already been unlocked |
| 9 | TemplateNotFound | Achievement template not found in catalog |
| 10 | NotEligible | Player does not meet achievement criteria |
| 11 | BadgeNotFound | Badge NFT does not exist |
| 12 | NonTransferable | Badge is soul-bound and cannot be transferred |
| 13 | NoPendingUpgrade | No upgrade has been authorized yet |
| 14 | NoRollbackTarget | No previous version to roll back to |

Soroban errors are returned as `ScError` values. In the JavaScript SDK, they surface as thrown `SorobanRpcError` objects — inspect `.code` against the table above.

## Endpoint groups

### Player

**Initialize a profile**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- initialize_profile \
  --owner $PLAYER_ADDRESS \
  --username "NebulaNomad"
```

**Get a profile**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- get_profile \
  --player $PLAYER_ADDRESS
```

### Ship

**Mint a ship NFT**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- mint_ship \
  --owner $PLAYER_ADDRESS \
  --ship_class "Explorer" \
  --metadata_uri "ipfs://Qm..."
```

**Get ship by ID**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- get_ship \
  --ship_id 42
```

### Achievements

**Check progress toward an achievement**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- get_achievement_progress \
  --player $PLAYER_ADDRESS \
  --achievement_id "FIRST_WARP"
```

**Unlock an achievement**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- unlock_achievement \
  --caller $PLAYER_ADDRESS \
  --achievement_id "FIRST_WARP"
```

**Batch-unlock achievements**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- batch_unlock_achievements \
  --caller $PLAYER_ADDRESS \
  --achievement_ids '["FIRST_WARP","NEBULA_SCOUT","RESOURCE_HUNTER"]'
```

### Nebula

**Generate a nebula layout**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- generate_nebula_layout \
  --seed $(echo -n "myseed00myseed00myseed00myseed00" | xxd -p | tr -d '\n')
```

### Governance

**Create a proposal**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- create_proposal \
  --proposer $PLAYER_ADDRESS \
  --title "Increase warp fuel cap" \
  --description "Raise the maximum fuel capacity from 100 to 150 units."
```

**Cast a vote**
```sh
stellar contract invoke \
  --id $CONTRACT_ID --network $NETWORK --source-account $SOURCE \
  -- cast_vote \
  --voter $PLAYER_ADDRESS \
  --proposal_id 1 \
  --approve true
```

## JavaScript SDK

```ts
import { Contract, SorobanRpc, TransactionBuilder, Networks } from "@stellar/stellar-sdk";

const server = new SorobanRpc.Server("https://soroban-testnet.stellar.org");
const contract = new Contract(process.env.CONTRACT_ID!);

// Read-only: get player profile
const tx = new TransactionBuilder(sourceAccount, { fee: "100", networkPassphrase: Networks.TESTNET })
  .addOperation(contract.call("get_profile", xdr.ScVal.scvAddress(...)))
  .setTimeout(30)
  .build();

const result = await server.simulateTransaction(tx);
```

## Related docs

- [Architecture overview](../ARCHITECTURE.md)
- [Yield math](../YIELD_MATH.md)
- [Upgrade guide](../UPGRADE_GUIDE.md)
