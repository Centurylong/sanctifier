#!/usr/bin/env bash
# Package the EXISTING, compiled browser WASM target and the browser demo as
# static assets without requiring npm registry access or a running API.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ ! -s dist/web/sanctifier_wasm_bg.wasm || ! -f dist/web/sanctifier_wasm.js ]]; then
  echo "No web WASM build found. Run ./scripts/build-npm.sh first." >&2
  exit 1
fi

out="dist/playground"
rm -rf "$out"
mkdir -p "$out/demo" "$out/js" "$out/dist/web"
cp demo/index.html demo/demo.js demo/demo.css demo/permalink.js "$out/demo/"
cp js/index.js "$out/js/"
cp dist/web/sanctifier_wasm.js dist/web/sanctifier_wasm_bg.wasm "$out/dist/web/"

# The existing demo references ../js/index.js, which imports ../dist/web.
# Preserving this path layout keeps the production bundle identical to dev.
cat > "$out/index.html" <<'HTML'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta http-equiv="refresh" content="0; url=./demo/">
    <title>Sanctifier browser playground</title>
  </head>
  <body><p><a href="./demo/">Open the browser playground</a></p></body>
</html>
HTML
echo "Static browser playground packaged at $out/"
echo "Serve with: python3 -m http.server 8080 --directory $out"
