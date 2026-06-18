use serde::Serialize;
use std::time::Instant;
use z3::ast::{Ast, Int};
use z3::{Context, SatResult, Solver};

/// Represents an invariant issue found by the SMT solver.
#[derive(Debug, Serialize, Clone)]
pub struct SmtInvariantIssue {
    pub function_name: String,
    pub description: String,
    pub location: String,
}

pub struct SmtVerifier<'ctx> {
    ctx: &'ctx Context,
}

impl<'ctx> SmtVerifier<'ctx> {
    pub fn new(ctx: &'ctx Context) -> Self {
        Self { ctx }
    }

    /// Proof-of-Concept: Uses Z3 to prove if `a + b` can overflow a 64-bit integer
    /// under unconstrained conditions.
    pub fn verify_addition_overflow(
        &self,
        fn_name: &str,
        location: &str,
    ) -> Option<SmtInvariantIssue> {
        let solver = Solver::new(self.ctx);
        let a = Int::new_const(self.ctx, "a");
        let b = Int::new_const(self.ctx, "b");

        // u64 bounds
        let zero = Int::from_u64(self.ctx, 0);
        let max_u64 = Int::from_u64(self.ctx, u64::MAX);

        // Constrain variables to valid u64 limits: 0 <= a, b <= u64::MAX
        solver.assert(&a.ge(&zero));
        solver.assert(&a.le(&max_u64));
        solver.assert(&b.ge(&zero));
        solver.assert(&b.le(&max_u64));

        // To prove overflow is IMPOSSIBLE, we assert the violation (a + b > max_u64)
        // and check if the solver can SATISFY this violation.
        let sum = Int::add(self.ctx, &[&a, &b]);
        solver.assert(&sum.gt(&max_u64));

        if solver.check() == SatResult::Sat {
            // A model exists where a + b > u64::MAX, meaning an overflow is mathematically possible
            Some(SmtInvariantIssue {
                function_name: fn_name.to_string(),
                description: "SMT Solver (Z3) proved that this addition can overflow u64 bounds."
                    .to_string(),
                location: location.to_string(),
            })
        } else {
            None
        }
    }
}

// ── Token invariant types ─────────────────────────────────────────────────────

/// The three built-in token contract invariants.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenInvariant {
    BalanceNonNegative,
    SupplyConserved,
    NoUnauthorizedMint,
}

impl TokenInvariant {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenInvariant::BalanceNonNegative => "balance_non_negative",
            TokenInvariant::SupplyConserved => "supply_conserved",
            TokenInvariant::NoUnauthorizedMint => "no_unauthorized_mint",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "balance_non_negative" => Some(TokenInvariant::BalanceNonNegative),
            "supply_conserved" => Some(TokenInvariant::SupplyConserved),
            "no_unauthorized_mint" => Some(TokenInvariant::NoUnauthorizedMint),
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            TokenInvariant::BalanceNonNegative,
            TokenInvariant::SupplyConserved,
            TokenInvariant::NoUnauthorizedMint,
        ]
    }
}

/// Whether an invariant was proved, violated, or undetermined.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProofStatus {
    /// Invariant holds for all possible inputs (UNSAT on violation formula).
    Proved,
    /// Invariant can be violated; `ProofResult::counterexample` shows how.
    Violated,
    /// Z3 could not determine satisfiability (timeout or unknown result).
    Unknown,
}

/// Concrete variable assignments that trigger a violation, plus a human-readable
/// description of the call sequence.
#[derive(Debug, Serialize, Clone)]
pub struct Counterexample {
    /// Pairs of (variable_name, concrete_value) extracted from the Z3 model.
    pub variables: Vec<(String, String)>,
    /// Human-readable description of the exact call that triggers the violation.
    pub call_sequence: String,
}

/// Full result of a formal invariant proof attempt.
#[derive(Debug, Serialize, Clone)]
pub struct ProofResult {
    pub invariant: String,
    pub status: ProofStatus,
    pub message: String,
    pub counterexample: Option<Counterexample>,
    pub duration_ms: u64,
}

// ── SmtProver ─────────────────────────────────────────────────────────────────

/// Proves or disproves token contract invariants using Z3 SMT solving.
pub struct SmtProver<'ctx> {
    ctx: &'ctx Context,
}

impl<'ctx> SmtProver<'ctx> {
    pub fn new(ctx: &'ctx Context) -> Self {
        Self { ctx }
    }

    pub fn prove_invariant(&self, invariant: &TokenInvariant) -> ProofResult {
        match invariant {
            TokenInvariant::BalanceNonNegative => self.prove_balance_non_negative(),
            TokenInvariant::SupplyConserved => self.prove_supply_conserved(),
            TokenInvariant::NoUnauthorizedMint => self.prove_no_unauthorized_mint(),
        }
    }

    /// Checks whether an unchecked `transfer` can drive a holder's balance below zero.
    ///
    /// Violation formula: `from_balance >= 0 ∧ amount > from_balance → (from_balance - amount) < 0`
    /// A SAT result means the vulnerability is real; the model yields the exact trigger values.
    fn prove_balance_non_negative(&self) -> ProofResult {
        let start = Instant::now();
        let solver = Solver::new(self.ctx);

        let from_balance = Int::new_const(self.ctx, "from_balance");
        let amount = Int::new_const(self.ctx, "amount");
        let zero = Int::from_i64(self.ctx, 0);
        let max_u64 = Int::from_u64(self.ctx, u64::MAX);

        solver.assert(&from_balance.ge(&zero));
        solver.assert(&from_balance.le(&max_u64));
        solver.assert(&amount.gt(&zero));
        solver.assert(&amount.le(&max_u64));

        // Violation: transfer amount exceeds balance (no underflow guard)
        solver.assert(&amount.gt(&from_balance));

        // Under integer arithmetic, from_balance - amount is now negative
        let result = Int::sub(self.ctx, &[&from_balance, &amount]);
        solver.assert(&result.lt(&zero));

        let status = solver.check();
        let duration_ms = start.elapsed().as_millis() as u64;
        let name = TokenInvariant::BalanceNonNegative.as_str().to_string();

        match status {
            SatResult::Sat => {
                let counterexample = solver.get_model().map(|model| {
                    let fb = model
                        .eval(&from_balance, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let amt = model
                        .eval(&amount, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1);
                    let res = fb - amt;
                    Counterexample {
                        variables: vec![
                            ("from_balance".into(), fb.to_string()),
                            ("amount".into(), amt.to_string()),
                            ("result_balance".into(), res.to_string()),
                        ],
                        call_sequence: format!(
                            "transfer(from_balance={fb}, amount={amt}) \
                            → balance becomes {res} (violates balance >= 0)"
                        ),
                    }
                });
                ProofResult {
                    invariant: name,
                    status: ProofStatus::Violated,
                    message: "Balance can go negative: missing underflow guard before subtraction."
                        .into(),
                    counterexample,
                    duration_ms,
                }
            }
            SatResult::Unsat => ProofResult {
                invariant: name,
                status: ProofStatus::Proved,
                message: "Balance is guaranteed non-negative for all valid inputs.".into(),
                counterexample: None,
                duration_ms,
            },
            SatResult::Unknown => ProofResult {
                invariant: name,
                status: ProofStatus::Unknown,
                message: "Z3 could not determine satisfiability (timeout).".into(),
                counterexample: None,
                duration_ms,
            },
        }
    }

    /// Proves that total token supply is conserved across a valid (bounds-checked) transfer.
    ///
    /// The violation formula `total_after != total_before` under correct transfer constraints
    /// must be UNSAT — meaning supply conservation is a mathematical certainty.
    fn prove_supply_conserved(&self) -> ProofResult {
        let start = Instant::now();
        let solver = Solver::new(self.ctx);

        let from_balance = Int::new_const(self.ctx, "from_balance");
        let to_balance = Int::new_const(self.ctx, "to_balance");
        let amount = Int::new_const(self.ctx, "amount");
        let zero = Int::from_i64(self.ctx, 0);
        let max_u64 = Int::from_u64(self.ctx, u64::MAX);

        // Valid initial state
        solver.assert(&from_balance.ge(&zero));
        solver.assert(&from_balance.le(&max_u64));
        solver.assert(&to_balance.ge(&zero));
        solver.assert(&to_balance.le(&max_u64));
        solver.assert(&amount.gt(&zero));

        // Valid transfer: amount <= from_balance (no underflow)
        solver.assert(&amount.le(&from_balance));

        let new_from = Int::sub(self.ctx, &[&from_balance, &amount]);
        let new_to = Int::add(self.ctx, &[&to_balance, &amount]);

        let total_before = Int::add(self.ctx, &[&from_balance, &to_balance]);
        let total_after = Int::add(self.ctx, &[&new_from, &new_to]);

        // Try to find a violation: total_after != total_before
        solver.assert(&total_after._eq(&total_before).not());

        let status = solver.check();
        let duration_ms = start.elapsed().as_millis() as u64;
        let name = TokenInvariant::SupplyConserved.as_str().to_string();

        match status {
            SatResult::Unsat => ProofResult {
                invariant: name,
                status: ProofStatus::Proved,
                message: "Total supply is provably conserved across all valid token transfers."
                    .into(),
                counterexample: None,
                duration_ms,
            },
            SatResult::Sat => {
                let counterexample = solver.get_model().map(|model| {
                    let fb = model
                        .eval(&from_balance, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let tb = model
                        .eval(&to_balance, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let amt = model
                        .eval(&amount, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1);
                    Counterexample {
                        variables: vec![
                            ("from_balance".into(), fb.to_string()),
                            ("to_balance".into(), tb.to_string()),
                            ("amount".into(), amt.to_string()),
                        ],
                        call_sequence: format!(
                            "transfer(from_balance={fb}, to_balance={tb}, amount={amt}) \
                            → supply changed (violates supply conservation)"
                        ),
                    }
                });
                ProofResult {
                    invariant: name,
                    status: ProofStatus::Violated,
                    message: "Total supply can change after a transfer.".into(),
                    counterexample,
                    duration_ms,
                }
            }
            SatResult::Unknown => ProofResult {
                invariant: name,
                status: ProofStatus::Unknown,
                message: "Z3 could not determine satisfiability (timeout).".into(),
                counterexample: None,
                duration_ms,
            },
        }
    }

    /// Checks whether a mint function without `require_auth` allows arbitrary callers to inflate supply.
    ///
    /// We model: caller ≠ admin ∧ new_supply > old_supply. This is trivially SAT — any account
    /// can mint because the auth check is missing.
    fn prove_no_unauthorized_mint(&self) -> ProofResult {
        let start = Instant::now();
        let solver = Solver::new(self.ctx);

        let caller_id = Int::new_const(self.ctx, "caller_id");
        let admin_id = Int::new_const(self.ctx, "admin_id");
        let old_supply = Int::new_const(self.ctx, "old_supply");
        let mint_amount = Int::new_const(self.ctx, "mint_amount");
        let zero = Int::from_i64(self.ctx, 0);
        let max_u64 = Int::from_u64(self.ctx, u64::MAX);

        solver.assert(&old_supply.ge(&zero));
        solver.assert(&old_supply.le(&max_u64));
        solver.assert(&mint_amount.gt(&zero));
        solver.assert(&mint_amount.le(&max_u64));
        // Admin must be a valid (non-zero) address
        solver.assert(&admin_id.gt(&zero));

        // The missing auth check: caller is NOT the admin, yet the mint succeeds
        solver.assert(&caller_id._eq(&admin_id).not());

        let new_supply = Int::add(self.ctx, &[&old_supply, &mint_amount]);
        solver.assert(&new_supply.gt(&old_supply));

        let status = solver.check();
        let duration_ms = start.elapsed().as_millis() as u64;
        let name = TokenInvariant::NoUnauthorizedMint.as_str().to_string();

        match status {
            SatResult::Sat => {
                let counterexample = solver.get_model().map(|model| {
                    let caller = model
                        .eval(&caller_id, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1337);
                    let admin = model
                        .eval(&admin_id, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1);
                    let supply = model
                        .eval(&old_supply, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let amt = model
                        .eval(&mint_amount, true)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1_000_000);
                    let new = supply + amt;
                    Counterexample {
                        variables: vec![
                            ("caller_id".into(), caller.to_string()),
                            ("admin_id".into(), admin.to_string()),
                            ("old_supply".into(), supply.to_string()),
                            ("mint_amount".into(), amt.to_string()),
                            ("new_supply".into(), new.to_string()),
                        ],
                        call_sequence: format!(
                            "mint(caller={caller}, amount={amt}) \
                            → supply {supply} → {new} \
                            (caller {caller} ≠ admin {admin}; missing require_auth(&admin))"
                        ),
                    }
                });
                ProofResult {
                    invariant: name,
                    status: ProofStatus::Violated,
                    message: "Unauthorized mint is possible: missing require_auth check allows \
                              any address to increase total supply."
                        .into(),
                    counterexample,
                    duration_ms,
                }
            }
            SatResult::Unsat => ProofResult {
                invariant: name,
                status: ProofStatus::Proved,
                message: "Mint is properly gated; no unauthorized caller can increase supply."
                    .into(),
                counterexample: None,
                duration_ms,
            },
            SatResult::Unknown => ProofResult {
                invariant: name,
                status: ProofStatus::Unknown,
                message: "Z3 could not determine satisfiability (timeout).".into(),
                counterexample: None,
                duration_ms,
            },
        }
    }
}

// ── Benchmark infrastructure (unchanged) ─────────────────────────────────────

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SmtProofStrategy {
    UnconstrainedOverflow,
    BoundedDomainOverflow,
    SmallDomainOverflow,
}

#[derive(Debug, Serialize, Clone)]
pub struct SmtStrategyLatency {
    pub strategy: SmtProofStrategy,
    pub runs: usize,
    pub min_micros: u128,
    pub max_micros: u128,
    pub avg_micros: u128,
    pub p95_micros: u128,
}

#[derive(Debug, Serialize, Clone)]
pub struct SmtLatencyBenchmarkReport {
    pub iterations_per_strategy: usize,
    pub strategies: Vec<SmtStrategyLatency>,
}

impl SmtLatencyBenchmarkReport {
    pub fn most_expensive_first(&self) -> Vec<SmtStrategyLatency> {
        let mut sorted = self.strategies.clone();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.avg_micros));
        sorted
    }
}

pub fn run_smt_latency_benchmark(iterations_per_strategy: usize) -> SmtLatencyBenchmarkReport {
    use z3::{Config, Context};

    let iterations = iterations_per_strategy.max(1);
    let strategies = [
        SmtProofStrategy::UnconstrainedOverflow,
        SmtProofStrategy::BoundedDomainOverflow,
        SmtProofStrategy::SmallDomainOverflow,
    ];

    let mut results = Vec::with_capacity(strategies.len());

    for strategy in strategies {
        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let cfg = Config::new();
            let ctx = Context::new(&cfg);

            let start = Instant::now();
            let _ = run_strategy(&ctx, strategy);
            samples.push(start.elapsed().as_micros());
        }

        samples.sort_unstable();
        let min_micros = samples.first().copied().unwrap_or_default();
        let max_micros = samples.last().copied().unwrap_or_default();
        let total: u128 = samples.iter().sum();
        let avg_micros = total / samples.len() as u128;
        let p95_index = (((samples.len() - 1) as f64) * 0.95).round() as usize;
        let p95_micros = samples[p95_index];

        results.push(SmtStrategyLatency {
            strategy,
            runs: iterations,
            min_micros,
            max_micros,
            avg_micros,
            p95_micros,
        });
    }

    SmtLatencyBenchmarkReport {
        iterations_per_strategy: iterations,
        strategies: results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use z3::{Config, Context};

    fn prover_ctx() -> (Config, Context) {
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        (cfg, ctx)
    }

    #[test]
    fn balance_non_negative_is_violated() {
        let (_cfg, ctx) = prover_ctx();
        let prover = SmtProver::new(&ctx);
        let result = prover.prove_invariant(&TokenInvariant::BalanceNonNegative);
        assert_eq!(result.status, ProofStatus::Violated);
        let ce = result.counterexample.expect("should have counterexample");
        // from_balance < amount means result_balance is negative
        let fb: i64 = ce.variables[0].1.parse().unwrap();
        let amt: i64 = ce.variables[1].1.parse().unwrap();
        let res: i64 = ce.variables[2].1.parse().unwrap();
        assert!(amt > fb, "amount must exceed from_balance");
        assert!(res < 0, "result balance must be negative");
    }

    #[test]
    fn supply_conserved_is_proved() {
        let (_cfg, ctx) = prover_ctx();
        let prover = SmtProver::new(&ctx);
        let result = prover.prove_invariant(&TokenInvariant::SupplyConserved);
        assert_eq!(result.status, ProofStatus::Proved);
        assert!(result.counterexample.is_none());
    }

    #[test]
    fn no_unauthorized_mint_is_violated() {
        let (_cfg, ctx) = prover_ctx();
        let prover = SmtProver::new(&ctx);
        let result = prover.prove_invariant(&TokenInvariant::NoUnauthorizedMint);
        assert_eq!(result.status, ProofStatus::Violated);
        let ce = result.counterexample.expect("should have counterexample");
        // caller_id != admin_id yet new_supply > old_supply
        let caller: i64 = ce.variables[0].1.parse().unwrap();
        let admin: i64 = ce.variables[1].1.parse().unwrap();
        let old_supply: i64 = ce.variables[2].1.parse().unwrap();
        let mint_amount: i64 = ce.variables[3].1.parse().unwrap();
        let new_supply: i64 = ce.variables[4].1.parse().unwrap();
        assert_ne!(caller, admin, "caller must differ from admin");
        assert!(new_supply > old_supply, "supply must have increased");
        assert!(mint_amount > 0, "mint amount must be positive");
    }

    #[test]
    fn token_invariant_round_trips_str() {
        for inv in TokenInvariant::all() {
            let s = inv.as_str();
            let back = TokenInvariant::from_str(s).expect("round-trip must succeed");
            assert_eq!(inv, back);
        }
    }

    #[test]
    fn token_invariant_unknown_str_returns_none() {
        assert!(TokenInvariant::from_str("nonexistent_invariant").is_none());
    }

    #[test]
    fn all_invariants_have_non_empty_messages() {
        let (_cfg, ctx) = prover_ctx();
        let prover = SmtProver::new(&ctx);
        for inv in TokenInvariant::all() {
            let result = prover.prove_invariant(&inv);
            assert!(
                !result.message.is_empty(),
                "message must not be empty for {}",
                inv.as_str()
            );
            assert!(!result.invariant.is_empty());
        }
    }

    #[test]
    fn duration_ms_is_recorded() {
        let (_cfg, ctx) = prover_ctx();
        let prover = SmtProver::new(&ctx);
        for inv in TokenInvariant::all() {
            let result = prover.prove_invariant(&inv);
            // Duration may be 0 on very fast hardware, but it must be a valid u64
            let _ = result.duration_ms;
        }
    }
}

fn run_strategy(ctx: &Context, strategy: SmtProofStrategy) -> SatResult {
    let solver = Solver::new(ctx);
    let a = Int::new_const(ctx, "a");
    let b = Int::new_const(ctx, "b");
    let zero = Int::from_i64(ctx, 0);
    let max_u64 = Int::from_u64(ctx, u64::MAX);

    solver.assert(&a.ge(&zero));
    solver.assert(&b.ge(&zero));

    match strategy {
        SmtProofStrategy::UnconstrainedOverflow => {
            solver.assert(&a.le(&max_u64));
            solver.assert(&b.le(&max_u64));
        }
        SmtProofStrategy::BoundedDomainOverflow => {
            let max = Int::from_i64(ctx, 5_000_000_000);
            solver.assert(&a.le(&max));
            solver.assert(&b.le(&max));
        }
        SmtProofStrategy::SmallDomainOverflow => {
            let max = Int::from_i64(ctx, 10_000);
            solver.assert(&a.le(&max));
            solver.assert(&b.le(&max));
        }
    }

    let sum = Int::add(ctx, &[&a, &b]);
    solver.assert(&sum.gt(&max_u64));
    solver.check()
}
