#!/usr/bin/env bash
# Build the @sanctifier/wasm npm package.
#
# Produces two wasm-bindgen targets under dist/ because the package supports
# both browsers and Node, and wasm-bindgen's glue differs between them: the
# `web` target needs an explicit async init and fetches the .wasm, while
# `nodejs` reads it from disk synchronously. js/index.js picks at runtime.
#
# Usage:  ./scripts/build-npm.sh [--skip-opt]
set -euo pipefail

cd "$(dirname "$0")/.."

WASM_PACK="${WASM_PACK:-wasm-pack}"
if ! command -v "$WASM_PACK" >/dev/null 2>&1; then
  echo "error: wasm-pack not found. Install it with:" >&2
  echo "  curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh" >&2
  exit 1
fi

OPT_FLAG=""
if [ "${1:-}" = "--skip-opt" ]; then
  # wasm-opt is the slow part of the build and is not needed to check that the
  # glue and the exports are correct, so CI smoke runs can skip it.
  OPT_FLAG="--no-opt"
fi

echo "==> building web target"
"$WASM_PACK" build --release --target web --out-dir pkg-web --out-name sanctifier_wasm $OPT_FLAG

echo "==> building nodejs target"
"$WASM_PACK" build --release --target nodejs --out-dir pkg-node --out-name sanctifier_wasm $OPT_FLAG

echo "==> assembling dist/"
rm -rf dist
mkdir -p dist/web dist/node

# Copy only the artifacts the package actually loads. wasm-pack also emits its
# own package.json and .gitignore into each pkg dir; ours is authoritative, so
# they are deliberately left behind.
for f in sanctifier_wasm.js sanctifier_wasm_bg.wasm sanctifier_wasm.d.ts sanctifier_wasm_bg.wasm.d.ts; do
  [ -f "pkg-web/$f" ] && cp "pkg-web/$f" dist/web/
  [ -f "pkg-node/$f" ] && cp "pkg-node/$f" dist/node/
done

# wasm-pack's nodejs target emits CommonJS (module.exports), but this package
# declares "type": "module", which would make node parse that glue as ESM and
# fail on the first `module.exports`. A nested manifest scopes CommonJS to just
# that directory; the web target stays ESM as wasm-pack emits it.
printf '{\n  "type": "commonjs"\n}\n' > dist/node/package.json

WEB_WASM_BYTES=$(wc -c < dist/web/sanctifier_wasm_bg.wasm | tr -d ' ')
echo "==> dist/web/sanctifier_wasm_bg.wasm: ${WEB_WASM_BYTES} bytes"
echo "==> done. Publish with: npm publish"
