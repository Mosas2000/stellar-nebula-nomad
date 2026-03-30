#!/usr/bin/env bash
set -euo pipefail

# Deployment script for Nebula Nomad contract to Soroban.
# Usage:
#   ./scripts/deploy.sh [network] [identity]
# Example:
#   ./scripts/deploy.sh futurenet default

NETWORK="${1:-futurenet}"
IDENTITY="${2:-default}"
WASM_PATH="target/wasm32-unknown-unknown/release/stellar_nebula_nomad.wasm"

command -v soroban >/dev/null 2>&1 || {
	echo "soroban CLI not found. Install with: cargo install soroban-cli --locked"
	exit 1
}

echo "==> Building WASM"
cargo build --target wasm32-unknown-unknown --release

echo "==> Optimizing WASM"
soroban contract optimize --wasm "$WASM_PATH"

echo "==> Deploying to network: $NETWORK (identity: $IDENTITY)"
CONTRACT_ID=$(soroban contract deploy \
	--wasm "$WASM_PATH" \
	--source-account "$IDENTITY" \
	--network "$NETWORK")

echo "Contract deployed: $CONTRACT_ID"

echo "==> Optional post-deploy smoke invoke: mint_ship"
echo "Run manually if needed:"
echo "soroban contract invoke --id $CONTRACT_ID --source-account $IDENTITY --network $NETWORK --fn mint_ship --owner <G...> --ship_type fighter --metadata 0x"

