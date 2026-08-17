# Tutorial: From Zero to a CI Security Gate

A step-by-step walkthrough that takes a new contributor from an empty
terminal to a Soroban contract repo where `sanctifier` fails CI on real
findings and stays quiet on everything else. Each command below is copied
verbatim from [`docs/cli.md`](cli.md) (the generated CLI reference), so if a
flag looks unfamiliar you can look it up there.

## Part 1 — Install

Pick any of the methods in [`docs/installation.md`](installation.md). The
fastest path on macOS/Linux:

```bash
brew tap Centurylong/sanctifier
brew install sanctifier
```

Confirm it's on your `PATH`:

```bash
sanctifier --help
```

## Part 2 — Run your first scan

From the root of a Soroban contract (a directory containing a `Cargo.toml`
with a `soroban-sdk` dependency):

```bash
sanctifier analyze
```

This walks every `.rs` file in the project and prints findings grouped by
category — storage collisions, auth gaps, panics, unchecked arithmetic,
ledger size warnings, and more. A clean project prints a `✅` line per
category; nothing to fix yet.

Try the machine-readable form, which is what CI will consume later:

```bash
sanctifier analyze --format json
```

## Part 3 — Understand a finding

Every finding carries a short code (e.g. `AUTH_GAP`, `ARITHMETIC_OVERFLOW`).
Each code has a dedicated writeup under
[`docs/detectors/`](detectors/README.md) explaining *why* it's flagged and
how to fix it — start there whenever a finding doesn't make sense. See also
[`docs/cookbook.md`](cookbook.md) for the vetted pattern that avoids the
issue in the first place.

## Part 4 — Suppress what you've already reviewed

Two mechanisms, for two different situations:

* **Inline, for a single reviewed line** — add a comment with a
  justification directly above the flagged code:

  ```rust
  // sanctifier-ignore: AUTH_GAP - callable only by the contract's own init routine, guarded elsewhere
  ```

* **Repo-wide baseline, for adopting Sanctifier on an existing codebase** —
  snapshot every current finding so only *new* findings fail CI going
  forward:

  ```bash
  sanctifier baseline
  ```

  This writes `.sanctify-baseline.json`. Commit it. Refresh it after an
  intentional change with `sanctifier baseline --update`.

## Part 5 — Wire the CI gate

Add a job that fails the build the moment a new, non-suppressed finding
shows up. `sanctifier ci` is purpose-built for this — it runs the same
analysis as `sanctifier analyze` but is scripted for gating (non-zero exit
on critical/high findings):

```yaml
# .github/workflows/sanctifier.yml
name: Sanctifier Security Gate

on:
  push:
    branches: ["main"]
  pull_request:
    branches: ["main"]

jobs:
  security-gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Sanctifier
        run: |
          brew tap Centurylong/sanctifier
          brew install sanctifier

      - name: Run security gate
        run: sanctifier ci
```

That's the whole gate: no findings (or only baselined/suppressed ones) means
green CI; a new `AUTH_GAP` or unchecked overflow means the PR is blocked
until it's fixed or explicitly suppressed with a justification.

## Part 6 — Validate end-to-end

Confirm the gate actually gates, on your own machine before trusting CI with
it:

1. Run `sanctifier baseline` on a clean project (empty or all findings
   suppressed) — `sanctifier ci` should exit `0`.
2. Introduce a deliberate issue (e.g. an `unwrap()` on a `require_auth()`
   result, or a raw arithmetic `+` on a balance) and re-run `sanctifier ci`
   — it should now exit non-zero and print the new finding.
3. Suppress it (inline comment or `sanctifier baseline --update`) and
   confirm `sanctifier ci` goes green again.

If all three steps behave as described, the gate is wired correctly and
ready to protect `main`.

## Where to go next

* [`docs/detectors/README.md`](detectors/README.md) — the full detector
  reference.
* [`docs/cookbook.md`](cookbook.md) — vetted patterns mapped to the
  detectors that enforce them.
* [`docs/cli.md`](cli.md) — every subcommand and flag, generated from the
  CLI itself so it never drifts.
