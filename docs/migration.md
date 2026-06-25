# Migration Guide: Adding Sanctifier to an Existing Soroban Repo

This guide takes a repository that **already has working Soroban contracts** and
adds Sanctifier to it in three stages:

1. **[First scan](#stage-1--first-scan)** — install the CLI and see where you stand.
2. **[Baseline](#stage-2--baseline)** — capture a known-good report and a README badge.
3. **[CI gate](#stage-3--ci-gate)** — fail pull requests that introduce critical/high findings.

It assumes nothing has been configured yet. By the end you will have a
`.sanctify.toml`, a committed baseline report, and a CI job that blocks
regressions. This page is part of the [core documentation set](README.md); pair
it with the [Configuration Reference](configuration.md) and the
[CLI Reference](cli.md).

> **Prerequisites:** a Rust toolchain (`rustc`/`cargo`), and your contracts build
> with `cargo build`. The `verify`/`prove` commands additionally need the Z3 SMT
> solver installed — see [Installing Z3](#installing-z3-for-verify-and-prove).

---

## Stage 1 — First scan

### 1.1 Install the CLI

From a clone of this repository:

```bash
cargo install --path tooling/sanctifier-cli
```

This puts a `sanctifier` binary on your `PATH`. Confirm it:

```bash
sanctifier --help
```

### 1.2 Scan a contract

Point `analyze` at a contract crate (a directory containing `Cargo.toml`, or the
workspace root). Paths default to `.`:

```bash
cd /path/to/your/soroban-repo
sanctifier analyze ./contracts/my-token
```

You will get a human-readable report of authorization gaps, panics, unchecked
arithmetic, ledger-size risks, and upgrade-pattern issues. Each finding maps to a
stable code (`S001`…`S016`) documented in [Finding Codes](error-codes.md); the
underlying terms are defined in the [Glossary](glossary.md).

> **Exit codes:** in the default **text** mode, `analyze` is informational and
> exits `0` even when it finds problems. To make a scan *fail* on
> critical/high-severity findings, use JSON mode (`--format json`) — see
> [Stage 3](#stage-3--ci-gate). This distinction is what lets you adopt
> Sanctifier gradually without breaking your build on day one.

### 1.3 Generate a config

Drop a starter `.sanctify.toml` into your repo root so the tool's behaviour is
explicit and version-controlled:

```bash
sanctifier init          # add --force to overwrite an existing file
```

Open the file and tune it using the [Configuration Reference](configuration.md).
At minimum, set [`ignore_paths`](configuration.md#ignore_paths) to skip vendored
or generated code, and decide on a [`ledger_limit`](configuration.md#ledger_limit)
and whether to enable [`strict_mode`](configuration.md#strict_mode).

Commit the file:

```bash
git add .sanctify.toml
git commit -m "chore: add Sanctifier configuration"
```

---

## Stage 2 — Baseline

A baseline is a machine-readable snapshot of the current findings. It lets you
(a) track progress as you fix issues and (b) render a status badge.

### 2.1 Capture a JSON report

```bash
sanctifier analyze . --format json > sanctifier-report.json
```

Inspect it (the report includes a full `error_codes` map plus per-category
`findings`):

```bash
jq '.summary, .error_codes' sanctifier-report.json
```

Decide what to do with the report:

- **Triage now:** fix the critical/high findings before wiring CI, so the gate in
  Stage 3 passes immediately.
- **Adopt gradually:** commit `sanctifier-report.json` as a baseline artifact and
  open follow-up issues for each finding, then turn the gate on once the count is
  manageable.

### 2.2 Add a README badge (optional)

Turn the JSON report into an SVG badge and a Markdown snippet:

```bash
sanctifier badge \
  --report sanctifier-report.json \
  --svg-output badges/sanctifier-security.svg \
  --markdown-output badges/sanctifier-security.md
```

Paste the generated Markdown into your README so the security status is visible.
See [`badge`](cli.md#sanctifier-badge) for all flags (including `--badge-url` for
hosting the SVG).

### 2.3 (Optional) Declare invariants

If you want formal guarantees, annotate token-style contracts with
`#[sanctify::invariant(...)]` and check them:

```bash
sanctifier verify ./contracts/my-token
```

See the [README invariants example](../README.md#verify-contract-invariants) and
`contracts/token-invariants` for a complete reference.

---

## Stage 3 — CI gate

Now make pull requests fail when they introduce critical or high-severity
findings. The key fact from Stage 1: **`analyze --format json` exits non-zero
when there are critical/high findings**, which is exactly what a CI step needs.

### 3.1 GitHub Actions

Add `.github/workflows/sanctifier.yml`:

```yaml
name: Sanctifier Security

on:
  pull_request:
  push:
    branches: ["main"]

jobs:
  sanctify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      # Required only if you also run `verify`/`prove` (Z3 SMT backend).
      - name: Install Z3
        run: sudo apt-get update && sudo apt-get install -y libz3-dev

      - name: Install Sanctifier
        run: cargo install --path tooling/sanctifier-cli
        # If Sanctifier lives in another repo, install from there instead, e.g.
        # cargo install --git https://github.com/Centurylong/sanctifier sanctifier-cli

      - name: Static analysis (fails on critical/high findings)
        run: sanctifier analyze . --format json > sanctifier-report.json

      - name: Upload report
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: sanctifier-report
          path: sanctifier-report.json

      # Optional: fail if any declared invariant is refuted or unknown.
      - name: Verify invariants
        run: sanctifier verify ./contracts --strict
```

The third step is the gate: a critical (e.g. missing `require_auth`) or high
(e.g. unchecked arithmetic, ledger size exceeded) finding makes the job exit
non-zero and blocks the merge. The `verify --strict` step adds a stronger gate
for contracts that declare invariants.

### 3.2 Tightening the gate over time

- Set [`strict_mode = true`](configuration.md#strict_mode) in `.sanctify.toml` to
  fail when state reaches 90% of the ledger limit.
- Lower [`approaching_threshold`](configuration.md#approaching_threshold) for
  earlier ledger-size warnings.
- Add project-specific [`custom_rules`](configuration.md#custom_rules) (reported
  as `S007`) to ban patterns your team has agreed to avoid.
- Add a `prove` smoke check for token invariants:

  ```bash
  sanctifier prove --invariant supply_conserved --no-save
  ```

---

## Installing Z3 (for `verify` and `prove`)

The `verify` and `prove` commands use the Z3 SMT solver. `analyze`, `badge`,
`init`, and `callgraph` do **not** require it.

| Platform | Command |
|----------|---------|
| Ubuntu/Debian | `sudo apt-get install -y libz3-dev` |
| macOS (Homebrew) | `brew install z3` |
| Fedora | `sudo dnf install z3-devel` |

If the build cannot find `z3.h` (common on macOS), point the bindings at your
install, e.g.:

```bash
export Z3_SYS_Z3_HEADER="$(brew --prefix z3)/include/z3.h"
```

See the [FAQ](faq.md#z3--dbus-build-errors) for more build-error fixes.

---

## Rollback

Sanctifier is additive and does not modify your contracts. To remove it:

```bash
rm .sanctify.toml sanctifier-report.json
rm -f .github/workflows/sanctifier.yml
cargo uninstall sanctifier-cli
```

---

## See also

- [Configuration Reference](configuration.md) — every `.sanctify.toml` key.
- [CLI Reference](cli.md) — every command and flag used above.
- [FAQ & Troubleshooting](faq.md) — fixes for common adoption problems.
- [Finding Codes](error-codes.md) — what each `S0xx` code means.
- [Glossary](glossary.md) — definitions for the security terms referenced here.
