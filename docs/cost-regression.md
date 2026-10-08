# Cost regression gate

`sanctifier-cost-gate` snapshots the existing production `GasEstimator` and
compares both instruction and memory estimates per source file, impl type,
and public method. It compiles only the estimator and parsing/JSON dependencies;
it does not run the security analyzer, an SMT solver, or a repository test suite.

Run from the repository root:

```sh
cargo run --locked --manifest-path tooling/sanctifier-cost-gate/Cargo.toml -- check .sanctify-cost-baseline.json 5
```

Exit 0 means all estimates are within the inclusive 5% threshold. Exit 1 means
an estimate regressed or a function was added/removed. Exit 2 means invalid
input, missing files, malformed Rust, an empty baseline, or another tool error.
CI must fail on either nonzero exit. Successful comparison emits JSON on stdout;
errors go to stderr. Thresholds use exact integer arithmetic, without float
rounding; a zero baseline cannot grow without review.

The committed baseline covers the actual SEP-41 token template in
`contracts/sep41-token-invariants/src/lib.rs`. Add explicit source files to
extend coverage. Source paths are relative to the repository root. Referenced
files must exist, parse as Rust, and remain inside that root. Directory recursion,
external modules and macro expansion are deliberately not inferred: list every
file whose public impl methods should be budgeted. Private helpers and top-level
free functions are outside the production estimator's current model. These are
heuristic relative costs, not measured network fees or execution budgets.

For an intentional cost change, regenerate and review the resulting JSON diff:

```sh
cargo run --locked --manifest-path tooling/sanctifier-cost-gate/Cargo.toml -- update .sanctify-cost-baseline.json contracts/sep41-token-invariants/src/lib.rs
```

Never regenerate automatically in CI: doing so accepts the regression. Review
baseline increases alongside the source rationale. Changes to the underlying
estimator also require deliberate baseline review. A new or removed method fails
comparison until its identity change is explicitly recorded.

`.github/workflows/cost-regression.yml` runs this one cost comparison on pull
requests. Maintainers must make the `cost-regression` check required in branch
protection/rulesets to block merging on failure; adding a workflow alone does
not grant or change repository administration settings. The threshold is a
literal in the workflow and should be reviewed like the baseline itself.
