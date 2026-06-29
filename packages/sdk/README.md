# @sanctifier/sdk

Typed JavaScript and TypeScript SDK that wraps the Sanctifier WASM analyzer for Stellar Soroban contracts. Works in Node 18+, modern browsers, and serverless runtimes.

This is the JS/TS counterpart to the Rust `sanctifier-cli`. It runs the same `sanctifier-core` rules, compiled to WebAssembly, so you can analyze Soroban source code from a Monaco playground, a VS Code extension, an edge function, or any other JS environment without spawning a Rust binary.

## Install

```bash
npm install @sanctifier/sdk
```

## Quick start (Node)

```ts
import { analyze } from "@sanctifier/sdk";

const source = await readSourceFromDisk();
const report = await analyze(source, { failOn: "high" });
report.findings.forEach((f) => console.log(f.code, f.severity, f.message));
```

When `failOn` is set, any finding at or above the given severity rejects the promise with a `SanctifierError` that carries the full report.

```ts
import { analyze, SanctifierError } from "@sanctifier/sdk";

try {
  await analyze(source, { failOn: "medium" });
} catch (err) {
  if (err instanceof SanctifierError) {
    console.error("Sanctifier flagged", err.report.summary.total, "issue(s).");
    process.exit(1);
  }
  throw err;
}
```

## Quick start (browser)

The browser entry needs the WASM bytes before any analysis call. The simplest form fetches the WASM file shipped next to the JS glue.

```ts
import { analyze, init } from "@sanctifier/sdk/browser";

await init();
const report = await analyze(sourceCode);
```

If your bundler does not auto-resolve the WASM URL, pass it explicitly.

```ts
import wasmUrl from "@sanctifier/sdk/wasm/web/sanctifier_bg.wasm?url";
import { init, analyze } from "@sanctifier/sdk/browser";

await init(wasmUrl);
```

See `examples/browser/index.html` for a full runnable demo that uses no bundler at all.

## API

### `analyze(source, options?)`

Runs every enabled rule on the given Soroban source string. Returns a `Promise<AnalysisReport>`.

| Field            | Type                         | Notes                                                  |
| ---------------- | ---------------------------- | ------------------------------------------------------ |
| `source`         | `string`                     | Raw Rust source. A single file is fine.                |
| `options.failOn` | `Severity`                   | Reject with `SanctifierError` when breached.           |
| `options.ledgerLimit` | `number`                | Override the ledger entry byte budget.                 |
| `options.approachingThreshold` | `number`      | Fraction of `ledgerLimit` that triggers a warning.     |
| `options.enabledRules` | `string[]`             | Limit to a subset of rule names.                       |
| `options.ignorePaths` | `string[]`              | Patterns to skip when running custom regex rules.      |
| `options.customRules` | `CustomRule[]`          | User defined regex rules with severity.                |

### `analyzeSync(source, options?)` (Node only)

Same as `analyze` but synchronous. Useful in CLI scripts where awaiting a promise adds friction.

### `findingCodes()`

Returns the catalog of stable finding codes (`S001`, `S002`, ...) with their category and description.

### `coreVersion()`

Returns the `sanctifier-core` semver embedded in the WASM bundle.

### `init(input?)`

Browser only. Loads and instantiates the WASM module. Call once before any analyze call. The Node entry exports a no-op `init` so isomorphic code can call it unconditionally.

## Report shape

```ts
interface AnalysisReport {
  findings: Finding[];
  summary: { total, critical, high, medium, low, info };
  raw: RawReport;
}

interface Finding {
  code: string;          // e.g. "S001"
  category: string;      // e.g. "authentication"
  severity: "info" | "low" | "medium" | "high" | "critical";
  message: string;
  location: string;
  function_name: string | null;
  line: number | null;
}
```

The `raw` field exposes the original per category vectors (auth gaps, panic issues, arithmetic issues, storage collisions, etc.) for callers that need finer grained access.

## Finding codes

The SDK speaks the same codes as `sanctifier-cli`. Call `findingCodes()` for the live catalog. The most common ones are:

| Code | Category        | What it means                                          |
| ---- | --------------- | ------------------------------------------------------ |
| S001 | authentication  | Mutating function missing `require_auth`               |
| S002 | panic_handling  | `panic!`, `unwrap`, or `expect` usage                  |
| S003 | arithmetic      | Unchecked arithmetic with overflow risk                |
| S004 | storage_limits  | Ledger entry near or above the size limit              |
| S005 | storage_keys    | Potential storage key collision                        |
| S009 | logic           | Unhandled `Result`                                     |
| S010 | upgrades        | Upgrade or admin mechanism without proper guards       |

## Local development

```bash
# From the repo root
cd packages/sdk

npm install
npm run build:wasm   # rebuilds the WASM bundles via wasm-pack
npm run build:ts     # bundles the TS sources with tsup
npm test             # runs the vitest suite against the built bundle
npm run example:node # runs examples/node/run.mjs
```

The `build:wasm` step requires `wasm-pack`. Install with `cargo install wasm-pack` or the official installer from `https://rustwasm.github.io/wasm-pack`.

## Compatibility

* Node 18 or later
* Evergreen browsers with WebAssembly and ES modules
* Bundlers: Vite, webpack 5, esbuild, Rollup, Parcel 2
* Serverless: Cloudflare Workers, Vercel Edge, Deno Deploy (browser bundle)

## License

MIT
