# Sanctifier browser playground

Analyze Soroban Rust contract source in a browser without a login, server API,
or an npm registry request at runtime. This playground uses the project's
existing **sanctifier-core** engine via its compiled **sanctifier-wasm**
WebAssembly binding. It is not a second set of client-only heuristics.

## Build, run and deploy

Build requires Rust, the wasm32-unknown-unknown target, and wasm-pack.

```bash
cd tooling/sanctifier-wasm
rustup target add wasm32-unknown-unknown
./scripts/build-npm.sh
bash ./scripts/package-playground.sh
python3 -m http.server 8080 --directory dist/playground
# Open http://localhost:8080/demo/
```

The page loads the real compiled WASM module, lets a user paste contract source,
runs local analysis, and renders severity totals, findings and elapsed time.
Use **Load sample** to reproduce security findings. Ctrl/Cmd+Enter also runs
the analyzer.

For public deployment, serve the **entire dist/playground/** output as static
assets over HTTPS, preserving the demo/, js/ and dist/web/ subdirectories.
Configure the web server to send .wasm with the application/wasm MIME type.
The bundle's index.html redirects to /demo/. No compiled WASM binary is
committed to this repository; build and package before deploying.

## Source permalink

Use **Share source link** to include up to 4 KiB of UTF-8 source in a versioned
URL fragment. The page displays the URL even when copying is blocked; secure
contexts can copy it to the clipboard. On opening a shared URL, the browser
restores the original source and analyzes it after the engine loads. Invalid
versions, encodings, empty or oversized sources are rejected.

The code fragment is not transmitted in an ordinary HTTP request, and the
playground makes no requests containing contract source. However, anyone who
receives a shared link can read the code inside it. It may also appear in
browser history, chats, logs of copied URLs, or screenshots. Do **not**
share secret or proprietary code using this feature.

## LSP boundary

The native Sanctifier LSP relies on process stdio, which cannot run as a
server inside a static browser page. The playground reuses the same underlying
sanctifier-core analyzers through WASM rather than inventing an incompatible
browser-LSP protocol. It does not claim native LSP quick-fixes.

## Manual acceptance smoke

1. Load the compiled example page, click Load sample then Analyze, and inspect
   its detailed findings and timing.
2. Generate a share link, reopen it in another browser tab, and compare source
   and results.
3. Include a multibyte UTF-8 character in the code, share again, and confirm
   exact restoration. Oversized input should produce a clear error.

These are executable operator instructions. They are not a claim of hosted
or browser smoke execution from this session.
