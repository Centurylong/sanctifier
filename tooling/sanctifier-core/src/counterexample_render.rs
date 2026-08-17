//! Turns a refuted invariant's [`Counterexample`](crate::smt::Counterexample)
//! into a concrete, reproducible Rust `#[test]` — the acceptance bar for
//! "actionable" counterexamples: a developer can paste the output straight
//! into their test suite and watch it fail for the same reason the solver
//! found.

#[cfg(feature = "smt")]
use crate::smt::Counterexample;

/// Render `ce` as a standalone `#[test]` function body asserting the
/// violated condition under the concrete variable assignment Z3 found.
///
/// The generated test is intentionally minimal (it doesn't know how to
/// construct a real contract call) — it documents the exact witness values
/// and the assertion they violate, so a developer can drop the values into
/// the relevant contract test harness.
#[cfg(feature = "smt")]
pub fn render_as_test(ce: &Counterexample, test_name: &str) -> String {
    let mut out = String::new();
    out.push_str("#[test]\n");
    out.push_str(&format!("fn {test_name}() {{\n"));
    out.push_str(&format!("    // {}\n", ce.call_sequence));
    for (name, value) in &ce.variables {
        out.push_str(&format!("    let {name} = {value};\n"));
    }
    out.push_str(&format!(
        "    // Counterexample: the following was expected to hold and did not.\n    assert!({});\n",
        ce.violated_assertion
    ));
    out.push_str("}\n");
    out
}

#[cfg(all(test, feature = "smt"))]
mod tests {
    use super::*;
    use crate::smt::{Counterexample, ProofStatus, SmtProver};
    use crate::smt::TokenInvariant;
    use z3::{Config, Context};

    #[test]
    fn renders_a_compilable_looking_reproduction_test() {
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let prover = SmtProver::new(&ctx);
        let result = prover.prove_invariant(&TokenInvariant::BalanceNonNegative);
        assert_eq!(result.status, ProofStatus::Violated);
        let ce: Counterexample = result.counterexample.expect("violated proof has a witness");

        let rendered = render_as_test(&ce, "repro_balance_non_negative_violation");

        assert!(rendered.starts_with("#[test]\n"));
        assert!(rendered.contains("fn repro_balance_non_negative_violation()"));
        assert!(rendered.contains("assert!("));
        for (name, _) in &ce.variables {
            assert!(
                rendered.contains(&format!("let {name} =")),
                "missing witness variable `{name}` in:\n{rendered}"
            );
        }
    }
}
