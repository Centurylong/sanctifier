#!/usr/bin/env bash
set -euo pipefail

# Rebuild the WASM bindings shipped with the SDK. Run this from the
# package root (packages/sdk) after touching the Rust sources in
# tooling/sanctifier-wasm. CI invokes it before npm run build:ts.

ROOT_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"
WASM_DIR="$ROOT_DIR/tooling/sanctifier-wasm"
OUT_DIR="$(cd "$(dirname "$0")/.." && pwd)/wasm"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "wasm-pack is required. Install with: cargo install wasm-pack" >&2
  exit 1
fi

echo "Building sanctifier-wasm (web target)..."
(cd "$WASM_DIR" && wasm-pack build --release --target web --out-dir pkg-web --out-name sanctifier)

echo "Building sanctifier-wasm (nodejs target)..."
(cd "$WASM_DIR" && wasm-pack build --release --target nodejs --out-dir pkg-node --out-name sanctifier)

rm -rf "$OUT_DIR/web" "$OUT_DIR/node"
mkdir -p "$OUT_DIR"
cp -r "$WASM_DIR/pkg-web" "$OUT_DIR/web"
cp -r "$WASM_DIR/pkg-node" "$OUT_DIR/node"

# wasm-pack ships a .gitignore that ignores everything, which also keeps npm
# from publishing the binaries. Drop it so the WASM ends up in the tarball.
rm -f "$OUT_DIR/web/.gitignore" "$OUT_DIR/node/.gitignore"

echo "WASM artifacts staged at $OUT_DIR"
