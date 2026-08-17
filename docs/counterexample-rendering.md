# Counterexample rendering

When Z3 refutes an invariant, [`ProofResult::counterexample`](../tooling/sanctifier-core/src/smt.rs)
already carries a minimized, concrete witness: variable assignments, the
assertion they violate, and a human-readable call sequence (see
`minimize_balance_counterexample` / `minimize_supply_counterexample` /
`minimize_mint_counterexample` in `smt.rs`, which shrink the raw Z3 model
toward the smallest values that still trigger the violation).

`sanctifier_core::counterexample_render::render_as_test` turns that witness
into a standalone, reproducible `#[test]` a developer can paste straight into
a test module:

```rust
use sanctifier_core::counterexample_render::render_as_test;
use sanctifier_core::smt::{SmtProver, TokenInvariant};
use z3::{Config, Context};

let cfg = Config::new();
let ctx = Context::new(&cfg);
let prover = SmtProver::new(&ctx);
let result = prover.prove_invariant(&TokenInvariant::BalanceNonNegative);

if let Some(ce) = result.counterexample {
    println!("{}", render_as_test(&ce, "repro_balance_non_negative_violation"));
}
```

produces:

```rust
#[test]
fn repro_balance_non_negative_violation() {
    // withdraw(amount) where from_balance=0, amount=1
    let from_balance = 0;
    let amount = 1;
    // Counterexample: the following was expected to hold and did not.
    assert!(from_balance - amount >= 0);
}
```

This is the same witness Z3 found, laid out as code instead of a `Vec<(String,
String)>` — no manual transcription from the `ProofResult` needed, and the
generated function name/assertion make it a starting point that can be
dropped into the relevant contract's test file and adjusted to call the real
entrypoint.
