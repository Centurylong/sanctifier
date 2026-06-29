# Good First Issue Starter Pack

Welcome! This is the curated list of well-scoped tasks for new contributors.
Each item links (or will link) to a dedicated issue. Pick one, comment to claim it, and ping the listed mentor if you get stuck.

> **Tip:** Read [CONTRIBUTING.md](../CONTRIBUTING.md) and the [Quick start](../CONTRIBUTING.md#quick-start) section before diving in.

---

## Starter tasks

| # | Title | Area | Difficulty | What you'll learn | Mentor |
|---|---|---|---|---|---|
| 1 | Add `--quiet` flag to CLI output | sanctifier-cli | easy | Clap argument parsing, CLI UX | open |
| 2 | Improve error messages for missing `require_auth` findings | sanctifier-core | easy | Finding formatting, Rust Display trait | open |
| 3 | Write a test for the `token-with-bugs` contract auth gap detection | sanctifier-core | easy | Integration testing, Soroban SDK test harness | open |
| 4 | Add `S008` finding: detect `unwrap_or_default` on auth calls | sanctifier-core | medium | AST traversal, finding codes | open |
| 5 | Add JSON output format to `sanctifier analyze` | sanctifier-cli | easy | serde_json, CLI flags | open |
| 6 | Document all CLI flags in `docs/cli-reference.md` | docs | easy | Reading Clap-generated help, markdown | open |
| 7 | Add a `--version` flag that prints the crate version at build time | sanctifier-cli | easy | `env!("CARGO_PKG_VERSION")`, Clap | open |
| 8 | Add proptest coverage for the AMM invariant `x * y = k` | contracts/amm-pool | medium | Property-based testing, proptest | open |
| 9 | Fix Clippy warnings in `contracts/reentrancy-guard` | contracts | easy | Clippy, Rust idioms | open |
| 10 | Add a GitHub Actions badge to the README | docs | easy | GitHub Actions status badges, markdown | open |
| 11 | Write an integration test for `runtime-guard-wrapper` health check | contracts | medium | Soroban test environment, integration testing | open |
| 12 | Add `S009` finding: detect integer overflow in arithmetic operations | sanctifier-core | medium | AST traversal, overflow detection patterns | open |

---

## How to claim a task

1. Open the corresponding issue (or open a new one using the **Good First Issue** template if it doesn't exist yet).
2. Leave a comment: "I'd like to work on this."
3. A maintainer will assign it to you within 48 hours.
4. Open a PR referencing the issue when you're ready.

---

## Resources

- [CONTRIBUTING.md](../CONTRIBUTING.md)
- [ARCHITECTURE.md](../ARCHITECTURE.md)
- [Soroban documentation](https://developers.stellar.org/docs/build/smart-contracts/getting-started)
- [Sanctifier finding codes](../docs/error-codes.md)
