# Mutation testing the detectors

Every detector in `sanctifier-core` is covered by a golden `insta` snapshot
(per `CONTRIBUTING.md`), which proves the detector's *current* output is
what's committed — but it can't prove the detector's test *fixtures* would
actually notice if the detector's logic broke. A snapshot test with only a
true-positive fixture and no false-negative/boundary fixture will happily
stay green even if a maintainer accidentally flips a `>` to `>=`, or deletes
a branch that used to be load-bearing.

Mutation testing closes that gap: it makes small, deliberate changes
("mutants") to a detector's source — flip a comparison, negate a
condition, swap a boolean literal — reruns the test suite, and reports
whether any test failed. A mutant that survives (no test caught it) is a
concrete, actionable signal: something in that function's logic has zero
test coverage of its *behavior*, only of its *output shape*.

## Running it

```bash
scripts/run-mutation-tests.sh
```

This installs [`cargo-mutants`](https://mutants.rs) if it isn't already
on `PATH`, then runs it (from the workspace root — that's where
`cargo-mutants` looks for `.cargo/mutants.toml`) against `sanctifier-core`
scoped to `tooling/sanctifier-core/src/rules/*.rs` (see
`.cargo/mutants.toml` — the engine code outside the detectors is
intentionally out of scope; see that file's comments for why).

To scope a run to one detector while iterating:

```bash
scripts/run-mutation-tests.sh --file tooling/sanctifier-core/src/rules/division_by_zero.rs
```

A full run across every detector is slow — mutation testing rebuilds and
retests the crate once per mutant, and there are 1000+ mutation sites
across the rules directory — which is why this is a manual / scheduled
job (`.github/workflows/mutation-testing.yml`), not a per-PR CI gate.
Scope a run to the file(s) you're actively touching instead of running
the full suite on every iteration.

## Reading the report

`cargo mutants` writes `mutants.out/` (at the workspace root) with
`caught.txt` (mutants a test killed — good, no action needed) and
`missed.txt` (survivors — the actual worklist). Each line names the
mutation site (file:line, and what changed, e.g. `replace > with >=`).

## Triaging survivors

Not every survivor is worth chasing. Triage each one into one of three
buckets:

1. **High-value: a real coverage gap.** The mutant changed behavior a
   detector should have caught — a boundary condition, a sign flip, a
   negated guard. Write a fixture (a small Soroban source snippet) that
   exercises the exact case the mutant broke, add it to the detector's
   test module, and regenerate the golden snapshot
   (`cargo insta test -p sanctifier-core --review`). This is the case the
   acceptance criterion "high-value survivors addressed" is about.
2. **Equivalent mutant.** The mutation doesn't actually change observable
   behavior for any reachable input (e.g. mutating an `unreachable!()`
   branch, or a constant only used in a `Debug` impl). No test can kill an
   equivalent mutant because there's nothing behaviorally different to
   catch. Leave it — chasing these wastes effort for zero coverage gain.
3. **Low-value: cosmetic-only.** The mutation changes a log/error message
   string, a `Debug`/display format, or similar output that isn't part of
   the detector's finding logic. Note it and move on; these don't
   represent security-relevant coverage gaps.

When in doubt, prefer bucket 1 — a test that turns out to be redundant
later costs far less than a real detector regression shipping silently.

## Why this crate, this scope

The detectors are the security-relevant surface: a detector with a
mutation-testing blind spot is a detector that could silently stop
catching the vulnerability class it exists for, and the golden-snapshot
suite alone would not notice. The shared engine (parsing, finding
plumbing, CLI) doesn't carry that same risk profile per line of code, so
scoping mutation runs to `src/rules/*.rs` keeps each run's signal
concentrated on what actually matters most to catch drifting.
