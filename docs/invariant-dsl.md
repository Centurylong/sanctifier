# Invariant DSL — Implementation Notes

## What was built

This document covers the implementation of `#[sanctify::invariant]` introduced
in PR #5 (closes issue #346).

## Files added / modified

| Path | Change |
|---|---|
| `tooling/sanctify-macros/` | New proc-macro crate |
| `tooling/sanctifier-core/src/invariant.rs` | New module: types, scanner, SMT verifier |
| `tooling/sanctifier-core/src/lib.rs` | +`pub mod invariant`, +`Analyzer::scan_invariant_attrs` |
| `tooling/sanctifier-core/tests/invariant_integration_test.rs` | 3 new integration tests |
| `tooling/sanctifier-cli/src/commands/verify.rs` | New `sanctifier verify` subcommand |
| `tooling/sanctifier-cli/src/commands/mod.rs` | +`pub mod verify` |
| `tooling/sanctifier-cli/src/main.rs` | +`Commands::Verify` variant and match arm |
| `contracts/token-invariants/` | New example contract |
| `ARCHITECTURE.md` | New Invariant DSL section |
| `README.md` | New `### Verify Contract Invariants` section |

## Acceptance criteria (from issue #346)

- [x] **Attribute parses invariant expressions.**
  `sanctify-macros::invariant` accepts any valid Rust expression and passes
  the impl block through unchanged in a normal build.

- [x] **Dispatches to Kani/SMT; reports results.**
  `SmtInvariantVerifier` handles integer equalities and tautologies via Z3.
  `cargo kani` is supported via the auto-generated `#[kani::proof]` harnesses.
  `sanctifier verify` renders PROVEN / REFUTED / UNKNOWN / KANI ↗.

- [x] **Example contract with at least one verified invariant.**
  `contracts/token-invariants` declares
  `#[invariant(pure::supply_is_conserved_after_transfer(0, 0, 0))]`
  and includes six Kani proof harnesses in `kani_proofs.rs`.

## Design decisions

**Why not emit a runtime assertion in the normal build?**
Soroban contracts panic on assertion failure, and the invariant expressions may
reference functions that are only meaningful in a testing context. The current
design keeps the production binary identical to the unannnotated version. A
future `--runtime-checks` flag could opt in to inline assertions.

**Why is `expr_tokens` kept in `InvariantArgs`?**
It is reserved for a future diagnostic mode that prints the token stream back
to the developer. It is not read in the current implementation.

**Why does `SmtInvariantVerifier` only handle integer literals and tautologies?**
Z3 can verify these patterns without needing to know the types or definitions
of the expressions, making the fast-path safe. The general case (user-defined
functions) requires either an Env stub or pure-function extraction, which is
the Kani path. This is the same split as the existing kani-poc contract.
