# @sanctifier/wasm

Soroban smart contract security analysis compiled to WebAssembly — runs in the
browser and in serverless edge runtimes, with no server round-trip and nothing
leaving the caller's machine.

This is the same `sanctifier-core` engine the CLI uses, wrapped in a thin JS
layer.

## Install

```sh
npm install @sanctifier/wasm
```

## Usage

```js
import { init, analyze } from "@sanctifier/wasm";

// One-time module load. Call it on page load so the compile cost does not
// land on the keystroke a user is waiting on. Safe to call repeatedly.
await init();

const findings = analyze(sourceCode);
// [{ code: "S001", category: "authentication", severity: "high",
//    message: "...", location: "line 12", line: 12 }]
```

### API

| Function | Returns | Notes |
|---|---|---|
| `init()` | `Promise<void>` | Loads the wasm module. Idempotent and concurrency-safe. Must resolve before anything below is called. |
| `analyze(source)` | `Finding[]` | The common case: just the findings. |
| `analyzeReport(source, config?)` | `AnalysisReport` | Findings plus a severity summary and the raw per-detector output. |
| `findingCodes()` | `FindingCode[]` | Every code this build can emit — useful for building filters without running an analysis first. |
| `version()` | `string` | Version of the underlying crate. |

Severities are lowercase: `"critical" | "high" | "medium" | "low" | "info"`.
Full types ship in [`js/index.d.ts`](js/index.d.ts).

Analysis is synchronous once `init()` has resolved. It runs on the calling
thread, so for very large inputs in a UI, call it from a worker.

## Building

Requires the pinned Rust toolchain (see `rust-toolchain.toml` at the repo root),
the `wasm32-unknown-unknown` target, and
[wasm-pack](https://rustwasm.github.io/wasm-pack/installer/).

```sh
rustup target add wasm32-unknown-unknown
./scripts/build-npm.sh          # full build, including wasm-opt
./scripts/build-npm.sh --skip-opt   # faster; skips size optimisation
npm test
```

`dist/` is a build artifact and is git-ignored — build before running the tests
or the demo.

### Why two targets

`dist/` holds both a `web` and a `nodejs` wasm-bindgen build, because their glue
differs: the browser build needs an explicit async init and fetches the `.wasm`,
while the Node build reads it from disk synchronously. `js/index.js` picks at
runtime, so callers never have to care which one they got.

`dist/node/package.json` pins that directory to `"type": "commonjs"` — wasm-pack
emits CommonJS for Node, and this package is otherwise ESM, so without the
nested manifest Node parses the glue as a module and fails on `module.exports`.

## Demo

A zero-dependency page that analyses pasted contract source entirely in the tab:

```sh
./scripts/build-npm.sh
npm run demo          # serves this directory on :8080
# open http://localhost:8080/demo/
```

It must be served over HTTP — `file://` cannot fetch the `.wasm`. The page
reports its own analysis time, and loads the same `js/index.js` entry point npm
consumers get, so if the demo works the published package works.

## Publishing

CI builds and verifies the tarball on every change but does not publish. To cut
a release, a maintainer with npm write access to the `@sanctifier` scope runs:

```sh
./scripts/build-npm.sh
npm publish     # package.json already sets access: public
```

Bump `version` in both `Cargo.toml` and `package.json` together — `version()`
reports the crate version, and the two drifting apart makes bug reports
ambiguous.
