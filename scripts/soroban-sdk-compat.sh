#!/usr/bin/env bash
#
# soroban-sdk-compat.sh — verify a specific soroban-sdk version compiles
# against the toolchain Sanctifier pins in CI (see .github/workflows/ci.yml).
#
# Sanctifier's workspace pins one soroban-sdk version (`^20.5.0`), but the
# detectors are meant to analyse contracts written against a range of SDK
# releases. Rather than mutating the workspace lockfile (which would force a
# single resolution), this script builds an *isolated probe crate* that depends
# on exactly the requested version. That keeps the check hermetic and lets CI
# fan the versions out across a matrix without disturbing the main build.
#
# Usage: scripts/soroban-sdk-compat.sh <soroban-sdk-version>
# Example: scripts/soroban-sdk-compat.sh 20.5.0
set -euo pipefail

VERSION="${1:?usage: soroban-sdk-compat.sh <soroban-sdk-version>}"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

echo ">> Probing soroban-sdk =$VERSION"

mkdir -p "$WORKDIR/src"

cat >"$WORKDIR/Cargo.toml" <<EOF
[package]
name = "soroban-sdk-compat-probe"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
soroban-sdk = "=$VERSION"

[workspace]
EOF

# A minimal contract that exercises the parts of the SDK surface the detectors
# reason about: #[contract]/#[contractimpl], Env, and a storage round-trip.
cat >"$WORKDIR/src/lib.rs" <<'EOF'
#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env};

#[contract]
pub struct CompatProbe;

#[contractimpl]
impl CompatProbe {
    pub fn bump(env: Env) -> u32 {
        let key = symbol_short!("n");
        let next: u32 = env.storage().instance().get(&key).unwrap_or(0) + 1;
        env.storage().instance().set(&key, &next);
        next
    }
}
EOF

( cd "$WORKDIR" && cargo build --quiet )

echo ">> soroban-sdk =$VERSION builds cleanly on $(rustc --version)"
