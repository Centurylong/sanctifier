#!/bin/bash

# Load environment variables from .env.local
if [ -f .env.local ]; then
    export $(cat .env.local | xargs)
fi

if [ -z "$SOROBAN_SECRET_KEY" ]; then
    echo "Error: SOROBAN_SECRET_KEY not found in .env.local"
    echo "Please create .env.local with SOROBAN_SECRET_KEY=S..."
    exit 1
fi

echo "Building contracts..."
cargo build --target wasm32-unknown-unknown --release

echo "Deploying vulnerable-contract..."
# Adjust the path to the WASM file as needed based on your package name in Cargo.toml
WASM_PATH="target/wasm32-unknown-unknown/release/vulnerable_contract.wasm"

if [ ! -f "$WASM_PATH" ]; then
    echo "Error: WASM file not found at $WASM_PATH"
    echo "Make sure the contract package name matches the expected WASM filename."
    exit 1
fi

# Deploy to Testnet (default)
echo "Deploying to Testnet using provided key..."
CONTRACT_ID=$(soroban contract deploy \
    --wasm "$WASM_PATH" \
    --source "$SOROBAN_SECRET_KEY" \
    --network testnet)

echo "Contract deployed successfully!"
echo "Contract ID: $CONTRACT_ID"
