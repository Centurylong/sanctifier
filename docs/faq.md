# FAQ & Troubleshooting

Common questions and fixes for Sanctifier. If you are just getting started, read
the [Migration Guide](migration.md) first; for flag details see the
[CLI Reference](cli.md) and for config keys the
[Configuration Reference](configuration.md). Security terms used below are
defined in the [Glossary](glossary.md).

**Jump to:** [General](#general) · [Installation & build](#installation--build) ·
[Configuration](#configuration) · [Analysis & findings](#analysis--findings) ·
[Formal verification](#formal-verification-verify--prove) ·
[CI/CD](#cicd) · [Error → fix table](#error--fix-table)

---

## General

### What is Sanctifier?

A security and formal-verification suite for [Stellar Soroban](glossary.md#soroban)
smart contracts. It combines static analysis (`analyze`), invariant checking
(`verify`), SMT-based proofs (`prove`), call-graph extraction (`callgraph`), and
reporting helpers (`badge`, `report`). See the [README](../README.md) for the
feature overview.

### Is Sanctifier a replacement for an audit?

No. It catches whole classes of mistakes early and cheaply, but it is a
complement to — not a substitute for — a professional audit and thorough testing.

### Which commands exist?

`analyze`, `badge`, `report`, `init`, `callgraph`, `update`, `verify`, `prove`.
The authoritative, always-current list (with every flag) is the
auto-generated [CLI Reference](cli.md).

### Does Sanctifier modify my contracts?

No. It reads source files and writes only the artifacts you ask for (reports,
badges, DOT graphs, proof certificates). Adoption is fully reversible — see
[Rollback](migration.md#rollback).

---

## Installation & build

### How do I install the CLI?

```bash
cargo install --path tooling/sanctifier-cli
```

### The build fails compiling `z3-sys` with `'z3.h' file not found`.

The `verify`/`prove` commands depend on the Z3 SMT solver, whose Rust bindings
need the Z3 C header at build time. Install Z3 and, if needed, point the bindings
at it:

```bash
# macOS
brew install z3
export Z3_SYS_Z3_HEADER="$(brew --prefix z3)/include/z3.h"

# Ubuntu/Debian
sudo apt-get install -y libz3-dev
```

Then rebuild. See the [error table](#z3--dbus-build-errors) for the dbus variant.

### The build fails with a `dbus`/`libdbus-1` error.

A transitive dependency needs the D-Bus development headers on Linux:

```bash
sudo apt-get update && sudo apt-get install -y libdbus-1-dev
```

This mirrors what the project's own CI installs (`libz3-dev libdbus-1-dev`).

### Do all commands need Z3?

No. Only `verify` and `prove` use the SMT backend. `analyze`, `badge`, `init`,
`callgraph`, and `report` work without it. (The CLI links Z3 as a hard
dependency today, so a clean build still needs the Z3 headers even if you only
plan to run `analyze`.)

### `sanctifier: command not found` after `cargo install`.

`cargo install` places binaries in `~/.cargo/bin`. Ensure that is on your `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

---

## Configuration

### Where do I put configuration, and how is it found?

In a file named `.sanctify.toml`. Sanctifier walks **up** from the path you scan
and uses the first one it finds, falling back to built-in defaults. Full details
in the [Configuration Reference](configuration.md#1-where-the-file-lives-and-how-it-is-found).

### I edited `.sanctify.toml` but nothing changed. Why?

Three usual causes:

1. **TOML syntax error.** A malformed file is silently ignored and defaults are
   used. The classic mistake is an unescaped backslash in a regex — write
   `"unsafe\\s*\\{"` or use a literal string `'unsafe\s*\{'`.
2. **A closer file shadows yours.** A `.sanctify.toml` nearer the scanned path
   wins over one higher up.
3. **The key isn't enforced the way you expect** — see the next question.

### I removed a rule from `enabled_rules` but it still runs.

That is expected today. The built-in analyzer runs its full detector set
regardless of `enabled_rules`; that key currently records *intent* and is
reserved for future per-rule gating. To guarantee a rule runs now, use
[`custom_rules`](configuration.md#custom_rules). See the
[behaviour note](configuration.md#enabled_rules).

### How do I add a project-specific rule?

Add a `[[custom_rules]]` table with a `name`, a regex `pattern`, and an optional
`severity` (`info`/`warning`/`error`). Matches are reported as `S007`. Example
and escaping rules: [`custom_rules`](configuration.md#custom_rules).

### How do I change the ledger size limit?

Set [`ledger_limit`](configuration.md#ledger_limit) in `.sanctify.toml`, or pass
`analyze --limit <bytes>` for a single run (the flag wins). Defaults to `64000`.

---

## Analysis & findings

### What does each `S0xx` code mean?

Codes `S001`…`S016` are listed in [Finding Codes](error-codes.md). Each
corresponds to a detector family (auth gaps, panics, arithmetic, ledger size,
storage collisions, custom rules, events, upgrades, SMT violations, and more).

### Does `analyze` fail my build when it finds something?

Only in JSON mode. `analyze --format json` exits non-zero when there are
**critical or high** findings; the default **text** mode is informational and
exits `0`. Use JSON mode for [CI gating](migration.md#stage-3--ci-gate).

### What counts as "critical" vs "high"?

- **Critical:** missing [`require_auth`](glossary.md#require_auth) on a
  state-mutating function, or an explicit `panic!`.
- **High:** unchecked arithmetic, any panic/unwrap/expect, unhandled `Result`,
  SMT invariant violations, or ledger state that exceeds the limit.

### Sanctifier flagged something that is actually safe. How do I handle a false positive?

Static analysis is conservative by design — it errs toward reporting. Options, in
order of preference:

1. **Refactor to remove ambiguity** — e.g. replace `unwrap()` with explicit error
   handling, or `+` with `checked_add`. This usually also improves the contract.
2. **Scope the scan** — use [`ignore_paths`](configuration.md#ignore_paths) to
   exclude generated code, fixtures, or vendored crates that you do not own.
3. **Treat text mode as advisory** — since text-mode `analyze` does not fail the
   build, you can review findings without blocking, and gate only on JSON mode
   once the signal is clean.

If you believe a detector is systematically wrong, open an issue with a minimal
reproduction; detectors are covered by golden snapshots, so regressions are
tracked.

### Why was a function flagged for an auth gap when it has `require_auth`?

The auth detector looks for `require_auth`/`require_auth_for_args` calls in
state-mutating functions. If the call is hidden behind a helper, a macro, or
indirection the static pass cannot follow, it may still flag the function.
Calling `require_auth` directly in the entry point resolves both the warning and
a real readability concern.

### Why are panics flagged even in tests?

The scanner walks `.rs` files under the scanned path. Exclude test-only paths via
[`ignore_paths`](configuration.md#ignore_paths) (e.g. add `"tests"` or your
snapshot directory) if you do not want test code analysed.

### What is "OOG" and how do I avoid it?

**OOG** = *Out Of Gas*: a contract invocation that exhausts its resource/CPU
budget and aborts. Sanctifier's ledger-size and resource heuristics
([`S004`](error-codes.md)) flag oversized state and patterns that inflate cost.
Reduce stored state, prefer `Temporary` storage where appropriate, avoid
unbounded loops over collections, and keep entries under
[`ledger_limit`](configuration.md#ledger_limit). See
[OOG](glossary.md#oog-out-of-gas).

### Does Sanctifier analyze compiled WASM?

No. Sanctifier performs **source-level** analysis of your Rust/Soroban code; it
parses `.rs` files, not the compiled `.wasm`. Build problems that only appear in
the `wasm32-unknown-unknown` target (e.g. a dependency that is not
`no_std`-compatible) are out of scope — fix those with `cargo build --target
wasm32-unknown-unknown`. See [WASM](glossary.md#wasm).

### How do I see cross-contract calls?

```bash
sanctifier callgraph ./contracts --output callgraph.dot
dot -Tsvg callgraph.dot -o callgraph.svg   # requires Graphviz
```

It extracts `env.invoke_contract` edges into a Graphviz DOT graph.

---

## Formal verification (`verify` & `prove`)

### What is the difference between `verify` and `prove`?

- `verify` scans for `#[sanctify::invariant(...)]` declarations across a contract
  or workspace and checks them. Pure-function invariants go to the Z3 SMT
  backend; complex ones are reported as `KANI ↗` with a reminder to run
  `cargo kani`.
- `prove` runs SMT proofs for a specific named token invariant
  (`balance_non_negative`, `supply_conserved`, `no_unauthorized_mint`, or `all`).

### How do I fail CI when an invariant cannot be proven?

```bash
sanctifier verify ./contracts --strict
```

`--strict` exits non-zero if any invariant is **Refuted** or **Unknown**.

### `verify`/`prove` says invariants are `Unsupported`.

The binary was built without the SMT backend, or Z3 is missing. Install Z3 (see
[Installation](#installation--build)) and rebuild.

---

## CI/CD

### What is the minimum CI gate?

A single step: `sanctifier analyze . --format json`. It fails on critical/high
findings. The full recommended workflow is in
[Migration → Stage 3](migration.md#stage-3--ci-gate).

### How do I keep the CLI reference from going stale?

This repository regenerates [`docs/cli.md`](cli.md) from the clap definitions and
fails CI on any diff. If you change a command or flag, regenerate it:

```bash
cargo run -p sanctifier-cli -- generate-docs > docs/cli.md
```

### Can I get a security badge for my README?

Yes — produce a JSON report and run [`badge`](cli.md#sanctifier-badge). See
[Migration → 2.2](migration.md#22-add-a-readme-badge-optional).

---

## Error → fix table

| Symptom / message | Likely cause | Fix |
|-------------------|--------------|-----|
| `cargo install` errors before any Sanctifier output | Missing Rust toolchain or old `cargo` | Install/update Rust via [rustup](https://rustup.rs/) |
| <a id="z3--dbus-build-errors"></a>`fatal error: 'z3.h' file not found` | Z3 dev headers not found by `z3-sys` | Install Z3; on macOS `export Z3_SYS_Z3_HEADER="$(brew --prefix z3)/include/z3.h"` |
| Build fails on `libdbus-1` / `dbus-sys` | D-Bus dev headers missing (Linux) | `sudo apt-get install -y libdbus-1-dev` |
| `sanctifier: command not found` | `~/.cargo/bin` not on `PATH` | `export PATH="$HOME/.cargo/bin:$PATH"` |
| Config edits have no effect | TOML parse error → defaults used, or shadowed by a closer file | Fix TOML (escape regex backslashes); check for a nearer `.sanctify.toml` |
| Custom regex rule never fires | Unescaped `\` in `pattern` | Use `"\\s"` or a literal string `'\s'`; see [`custom_rules`](configuration.md#custom_rules) |
| `analyze` reports issues but CI still passes | Text mode is informational | Gate with `analyze --format json` (exits non-zero on critical/high) |
| Ledger-size warnings you consider noise | `approaching_threshold` too low | Raise [`approaching_threshold`](configuration.md#approaching_threshold), or fix state size |
| Invariants reported as `Unsupported` | Built without SMT backend / no Z3 | Install Z3 and rebuild |
| `verify`/`prove` exits non-zero in CI | An invariant was Refuted/Unknown (with `--strict`) | Fix the invariant, or investigate the counterexample |
| Contract builds for host but not `wasm32` | Non-`no_std` dependency | This is a build issue, not a Sanctifier finding — fix the dependency; see [WASM](glossary.md#wasm) |
| Out-of-gas on invocation (OOG) | Oversized state / unbounded work | Shrink state, use `Temporary` storage, bound loops; see [OOG](glossary.md#oog-out-of-gas) |
| False positive auth gap | `require_auth` hidden behind indirection | Call `require_auth` directly in the entry point |

---

## See also

- [CLI Reference](cli.md) · [Configuration Reference](configuration.md) ·
  [Migration Guide](migration.md) · [Finding Codes](error-codes.md) ·
  [Glossary](glossary.md)
