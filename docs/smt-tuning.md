# Z3 tactic tuning + timeout/resource controls

SMT performance determines how much sanctifier-core can prove per CI run: an
unbounded query can eat an entire job's budget on one pathological invariant,
starving every other proof queued behind it.

## Configuring the timeout

Every Z3 `Context` in this crate should be built through
[`smt::configured_z3_config`](../tooling/sanctifier-core/src/smt.rs) rather
than a bare `z3::Config::new()`:

```rust
use sanctifier_core::smt::{configured_z3_config, DEFAULT_Z3_TIMEOUT_MS};
use z3::Context;

let cfg = configured_z3_config(DEFAULT_Z3_TIMEOUT_MS); // 5_000ms
let ctx = Context::new(&cfg);
```

Pass a different budget for callers with different needs — a slower CI
runner, or a tight local iteration loop:

```rust
let cfg = configured_z3_config(1_000); // fail fast locally
```

`DEFAULT_Z3_TIMEOUT_MS` (5 seconds) is the budget used by
[`run_smt_latency_benchmark`](../tooling/sanctifier-core/src/smt.rs), the
existing per-strategy latency harness (`SmtProofStrategy::{UnconstrainedOverflow,
BoundedDomainOverflow, SmallDomainOverflow}`) that reports min/avg/p95 proving
time per strategy over N iterations.

## Measuring proving-rate impact

Run the benchmark to get concrete numbers for a given change:

```rust
use sanctifier_core::smt::run_smt_latency_benchmark;

let report = run_smt_latency_benchmark(50);
for strategy in report.most_expensive_first() {
    println!("{:?}: avg={}us p95={}us", strategy.strategy, strategy.avg_micros, strategy.p95_micros);
}
```

On this crate's three built-in strategies, bounding the domain
(`BoundedDomainOverflow`, `SmallDomainOverflow`) rather than leaving
variables unconstrained (`UnconstrainedOverflow`) is consistently the
larger lever on proving time — a tighter domain gives Z3 far less search
space regardless of the timeout value. The timeout budget above is a
backstop against the cases that don't respond to domain-narrowing, not a
substitute for it.

## Guidance for new proofs

- Narrow variable domains as tightly as the invariant allows before falling
  back to a larger timeout — it's the more reliable lever.
- Use `configured_z3_config` everywhere a `Context` is constructed so the
  timeout stays centrally adjustable.
- Re-run `run_smt_latency_benchmark` after changing a proof's shape and note
  the before/after numbers in the PR description.
