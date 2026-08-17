# Stellar/Soroban Ecosystem Catalog Listings

Tracks getting Sanctifier listed in the tooling directories the Stellar and
Soroban developer community actually browses, so contract authors discover it
organically instead of only through this repo. For issue #776.

Submitting to third-party catalogs (a PR against *their* repo, or a form on
*their* site) is out of scope for a single contributor PR to do end-to-end —
most require a maintainer to open/sign the submission. This doc is the
tracked checklist plus the ready-to-paste listing copy, so any maintainer (or
future contributor) can complete a row in one sitting.

## Target catalogs

| Catalog | Type | Status | Notes |
| --- | --- | --- | --- |
| [Stellar Developer Resources](https://developers.stellar.org/community/dev-tools) | Official dev-tools directory | Not yet submitted | Submission is a PR against the [stellar-docs](https://github.com/stellar/stellar-docs) repo, "Developer Tools" section |
| [Awesome Soroban](https://github.com/topics/soroban) (GitHub topic) | Topic tag | Not yet applied | Add `soroban` + `soroban-security` + `static-analysis` topics to this repo's GitHub settings |
| [Soroban Ecosystem](https://soroban.stellar.org/ecosystem) | Official ecosystem page | Not yet submitted | Check current submission process on the page before opening a request |
| [crates.io](https://crates.io/crates/sanctifier-core) | Rust package registry | Not yet published | Requires publishing `sanctifier-core`/`sanctifier-cli` crates (separate from GitHub listing; blocks discovery via `cargo search`) |
| [Awesome Rust Security](https://github.com/rust-secure-code/awesome-rust-security) | Curated list | Not yet submitted | Community-maintained; submission is a PR adding one line under "Static Analysis" |

This repo already maintains its own [`docs/awesome-soroban-security.md`](awesome-soroban-security.md)
curated list (which self-lists Sanctifier) — that's a different thing from
*this* checklist, which is about getting listed in catalogs Sanctifier
doesn't control.

## Listing copy (paste-ready)

**One-liner:**
> Sanctifier — static analysis and formal-verification suite for Stellar Soroban smart contracts. Detects auth gaps, storage collisions, arithmetic overflow/underflow, panics, and denial-of-service patterns via `syn`-based AST rules, with Kani/Z3 formal-verification integration.

**Short description (for directories with a 1-2 sentence limit):**
> A Rust static-analysis CLI purpose-built for Soroban contracts: 30+ detectors covering authentication, arithmetic, storage, and DoS vulnerability classes, plus formal verification via Kani and Z3. See the [detector catalog](detectors/README.md).

**Links to include:** repository (`https://github.com/Centurylong/sanctifier`), [documentation index](README.md), [detector catalog](detectors/README.md).

## Verifying links

Before checking a row off as "listed," confirm:

- [ ] The listing links to the canonical repo URL, not a fork
- [ ] The listing's description matches (or is a reasonable paraphrase of) the copy above
- [ ] The link resolves (no 404) as of the check date — note the date in this table when updating a row

## See also

- [`docs/awesome-soroban-security.md`](awesome-soroban-security.md) — Sanctifier's own curated ecosystem list
- [`docs/positioning.md`](positioning.md) — how Sanctifier is positioned relative to other tools
