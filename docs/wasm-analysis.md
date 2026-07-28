# Source-Optional WASM Analysis

Some audit targets ship only as a deployed `.wasm` module — no Rust source. The
`sanctifier wasm` command analyzes that compiled artifact directly, running a
subset of checks that do not need the source. This page documents what it does
and, importantly, **what it cannot do compared to source mode** (`sanctifier
analyze`), which is the acceptance criterion for issue #778.

## Usage

```bash
# Build a contract to WASM, then analyze the artifact.
sanctifier wasm ./target/wasm32-unknown-unknown/release/my_contract.wasm

# Machine-readable output for CI.
sanctifier wasm ./my_contract.wasm --format json

# Print the full source-vs-WASM limitations note.
sanctifier wasm ./my_contract.wasm --show-limitations
```

The command reads the module, prints a **structural summary** (functions,
imports, exports, memory, Soroban metadata sections) and any **bytecode-level
findings**, then exits non-zero if any error-severity finding is present so it
can gate CI like `analyze`.

## What it checks

The parser is hand-rolled (zero extra dependencies) and walks the module's
sections. Every check is derived from section structure alone — no semantic or
opcode-level reasoning:

| Code | Severity | What it means |
|------|----------|---------------|
| [`W001`](error-codes.md) | Warning | No `contractspecv0` custom section — the module may not be a deployable Soroban contract, or was built without the SDK. |
| [`W002`](error-codes.md) | Warning | The module exports no callable functions, so nothing can be invoked as a contract entrypoint. |
| [`W003`](error-codes.md) | Info | No `contractenvmetav0` section — the target interface version is unknown, so SDK/protocol compatibility can't be verified. |
| [`W004`](error-codes.md) | Error | Function signatures use `f32`/`f64` value types, which the Soroban host forbids; the module will be rejected or trap. |

It also recovers and reports: the number of defined functions, imported
functions and their module names (typically `env`), exported function names,
declared memory pages, and which custom sections are present.

## Limitations vs. source mode

Bytecode analysis trades reach for depth. Once a contract is compiled, the
symbols, types, and macro expansions the source-based detectors rely on are gone.
`sanctifier wasm` is explicit about this — it prints a limitations note on every
run, and the full list is:

- **No authentication-gap detection ([`S001`](error-codes.md)).**
  `require_auth()` compiles to a host-function import call that is
  indistinguishable from any other `env` call at the bytecode level.
- **No arithmetic-overflow detection ([`S003`](error-codes.md)).** Source-level
  `+`/`-`/`*` lower to `i128` helper calls or `i64` opcodes with no type or
  variable context to flag.
- **No storage-key-collision, event, or upgrade-pattern analysis**
  ([`S005`](error-codes.md), [`S008`](error-codes.md),
  [`S010`](error-codes.md)). These rely on source symbols and macros that don't
  survive compilation.
- **No line-accurate locations.** Findings map to the module as a whole, not to
  a `file:line`.
- **Names are limited.** Only the exported entrypoints have names (and only when
  a name section or contract spec is present); internal function and argument
  names are erased.

### When to use which

| | Source mode (`analyze`) | Source-optional (`wasm`) |
|---|---|---|
| Input | Rust source / workspace | Compiled `.wasm` |
| Detector coverage | Full catalog (see [detectors](detectors/README.md)) | Structural checks `W001`–`W004` |
| Locations | `file:line` | Module-level |
| Use when | You have the repo | You only have the deployed artifact |

**Rule of thumb:** prefer `analyze` whenever the source is available. Reach for
`wasm` for third-party artifacts, deployed-contract triage, or a fast
integrity/shape check of a build output — and treat a clean `wasm` run as
"nothing obvious at the bytecode level," not "audited."

## See also

- [CLI Reference](cli.md) — the generated `sanctifier wasm` flag reference.
- [Finding Codes](error-codes.md) — the `W0xx` family and the source-mode codes.
- [Detector Catalog](detectors/README.md) — the full source-mode checks.
