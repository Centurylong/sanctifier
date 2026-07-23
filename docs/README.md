# Sanctifier Documentation

The core documentation set for [Sanctifier](../README.md) — the security and
formal-verification suite for [Stellar Soroban](https://soroban.stellar.org/)
smart contracts. These pages are written as one coordinated body of work: shared
structure, a consistent voice, and complete cross-linking, so a newcomer can
adopt Sanctifier unaided.

## Start here

New to Sanctifier and adding it to an existing project? Read in this order:

1. **[Migration Guide](migration.md)** — install, run a first scan, capture a
   baseline, and gate CI. The fastest path from zero to a working setup.
2. **[CLI Reference](cli.md)** — every command and flag. *Auto-generated from the
   clap definitions and verified in CI, so it never drifts from the parser.*
3. **[Configuration Reference](configuration.md)** — every `.sanctify.toml` key,
   its type, default, and precedence, with an annotated sample.
4. **[FAQ & Troubleshooting](faq.md)** — answers to common questions plus an
   error → fix table (install, Z3/dbus, OOG, WASM, false positives).
5. **[Glossary](glossary.md)** — 50 Soroban/Stellar security terms with stable
   anchors that findings and reports can deep-link to.

## Reference

- **[Positioning](positioning.md)** — where Sanctifier fits relative to a manual
  audit and other tools, with an honest scope and limitations statement.
- **[Finding Codes](error-codes.md)** — the `S001`…`S016` codes emitted in CLI and
  JSON output.
- **[Detector Catalog](detectors/README.md)** — one page per detector: what it
  catches, a vulnerable example, the fix, and references. Coverage is enforced in
  CI.
- **[Getting Started (detailed)](getting-started.md)** — example output and
  finding-by-finding explanations.
- **[Detector Cookbook](detector-cookbook.md)** — three worked examples of custom detectors (trivial, medium, and data-flow).
- **[soroban-sdk Compatibility](soroban-sdk-compatibility.md)** — the supported
  soroban-sdk versions, the CI compatibility matrix, and version-specific
  handling (including the toolchain pin).
- **[Talks & Workshops](talks/README.md)** — ready-to-present slide deck and
  live-demo script for demonstrating Sanctifier at conferences and workshops.
- **[Awesome Soroban Security](awesome-soroban-security.md)** — curated external
  tools, audits, incidents, and learning resources.
- **[Differential Testing vs Slither/Aderyn](differential-testing.md)** — how
  Sanctifier's coverage compares to established EVM analyzers on overlapping
  checks, with the shared corpus, the overlap matrix, and follow-up gaps.
- **[Differential Testing vs other Soroban linters](differential-soroban-linters.md)**
  — the same-platform study vs Soroban-native analyzers (CoinFabrik Scout), a
  detector-for-detector overlap matrix, agreements/gaps, and follow-ups.

## How these pages fit together

```text
Migration ──▶ CLI Reference ──▶ Configuration
    │              │                  │
    └──────────────┴───────┬──────────┘
                           ▼
                  FAQ  ◀──▶  Glossary
                           ▲
                    Finding Codes
```

Every page links to the others, and findings deep-link into the
[Glossary](glossary.md) (e.g. `glossary.md#require_auth`) and
[Finding Codes](error-codes.md). If you change a command or flag, regenerate the
CLI reference so CI stays green:

```bash
cargo run -p sanctifier-cli -- generate-docs > docs/cli.md
```
