# Sanctifier Rule Engine

The Rule Engine provides a structured way to execute multiple security and correctness checks on Soroban contract source code.

## Component Overview

Located in `tooling/sanctifier-cli/src/rules/`, the `RuleEngine` wraps the various scanners provided by the `sanctifier-core` library.

### Core Architecture

- **`RuleEngine`**: Orchestrates the analysis process, calling each scanner in sequence and aggregating results.
- **`run_all`**: Executes all enabled rules on a given source string, optionally tagging results with a file path for improved reporting.
- **Cached Analysis**: Analysis results are stored in `.sanctifier_cache.json` using the `CachedAnalysis` structure.

## Enabled Rules

The engine currently supports the following analysis categories:

1. **Auth Gaps**: Detects state-mutating functions missing `require_auth()`.
2. **Panic Risks**: Flags explicit `panic!`, `.unwrap()`, and `.expect()` calls.
3. **Arithmetic Overflow**: Identifies bare arithmetic operators (`+`, `-`, `*`) that lack overflow protection.
4. **Ledger Size**: Estimates the serialized size of `#[contracttype]` structures and warns if approaching limits.
5. **Reentrancy Risks**: Detects state mutations combined with external calls without reentrancy guards.
6. **Deprecated APIs**: Warns about usage of legacy Soroban host functions.
7. **Custom Rules**: Allows user-defined regex patterns from `.sanctify.toml`.
8. **Gas Estimation**: Provides heuristic-based instruction and memory cost estimations.

## Testing Strategy

The rule engine is validated using a comprehensive suite of "mock Soroban ASTs" (source code snippets) located in `tooling/sanctifier-cli/src/rules/tests.rs`. These tests ensure that each rule correctly identifies its target vulnerability type and provides accurate location mapping.

To run the rule engine tests:

```bash
cargo test -p sanctifier-cli rules::tests::tests
```
