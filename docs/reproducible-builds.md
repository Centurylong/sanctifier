# Reproducible Builds (issue #754)

A reproducible build means: given the same source commit, anyone — a
developer, an auditor, a validator — can rebuild the deployed WASM and get
byte-identical output. That's what lets a third party verify that the bytecode
running on-chain actually matches the source it claims to be built from,
without trusting the deployer's word for it.

Two inputs have to be pinned for that to hold, and both are easy to leave
floating by accident:

1. **The Rust toolchain.** `rustc` isn't guaranteed to emit identical code
   across versions (or even patch releases, in principle). If your project has
   no `rust-toolchain.toml`, `cargo build` uses whatever toolchain happens to
   be active — different on every machine and liable to drift on CI runners
   over time.
2. **The `soroban-sdk` version.** Cargo's default dependency requirement is a
   caret range: `soroban-sdk = "20.5.0"` really means `^20.5.0`, i.e. "20.5.0
   or any later 20.x". A plain `cargo update` between two builds of the exact
   same commit can silently pull in a newer SDK release and change the
   compiled output.

## Checking a project

```bash
sanctifier toolchain --path .
sanctifier toolchain --path . --json   # machine-readable
```

This reports two advisories when applicable:

- `toolchain_pin` — no `rust-toolchain.toml`/`rust-toolchain`, or one that
  pins a floating channel (`stable`, `nightly`, or a two-part version like
  `1.85`) instead of an exact release (`1.85.0`).
- `sdk_pin` — a `soroban-sdk` dependency (in any `Cargo.toml` under the given
  path) resolved through a floating version requirement instead of an exact
  `=` pin.

It's advisory only — the command always exits `0`. The goal is a report a
developer or CI job reads, not an extra build gate.

## Fixing it

**Pin the toolchain** with a `rust-toolchain.toml` at the workspace root:

```toml
[toolchain]
channel = "1.85.0"
components = ["rustfmt", "clippy"]
```

`rustup` reads this automatically and installs/selects that exact toolchain
before any `cargo` command runs — no per-developer setup needed. This
repository's own `rust-toolchain.toml` pins the same `1.85.0` release
`.github/workflows/ci.yml` already pins via `dtolnay/rust-toolchain@1.85.0`,
so local builds and CI builds now use the identical compiler by construction.

**Pin the SDK** with an exact version requirement:

```toml
[dependencies]
soroban-sdk = "=20.5.0"
```

Or, in a workspace with multiple contract crates (as this repo has), pin it
once in `[workspace.dependencies]` and have every crate inherit it with
`soroban-sdk = { workspace = true }` — one place to bump, no crate can drift
independently.

## What this doesn't cover

Pinning the toolchain and SDK gets you deterministic *compiler input*. Fully
reproducing the exact `.wasm` bytes on-chain also depends on things this
advisory doesn't check yet — build flags (`RUSTFLAGS`, `wasm-opt` version and
options), the C toolchain used by any native build dependencies, and the host
OS/architecture. Cross-checking a locally rebuilt `.wasm` against the deployed
module is exactly what
[`sanctifier wasm`](wasm-analysis.md)'s source-optional mode is for.
