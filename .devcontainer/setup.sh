#!/usr/bin/env bash
set -e

echo "==> Installing system dependencies"
sudo apt-get update -q
sudo apt-get install -y -q libz3-dev libdbus-1-dev libudev-dev pkg-config

echo "==> Adding wasm32 target"
rustup target add wasm32-unknown-unknown

echo "==> Installing Soroban CLI"
cargo install --locked soroban-cli || true

echo "==> Installing cargo tools"
cargo install cargo-tarpaulin || true

echo "==> Done. Run 'cargo build -p sanctifier-cli' to verify."
