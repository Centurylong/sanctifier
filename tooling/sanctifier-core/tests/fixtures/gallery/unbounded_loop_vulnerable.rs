#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Vec};

// GALLERY: unbounded loop
// Mapped finding: S006 (unsafe_pattern) — planned detector.

#[contract]
pub struct UnboundedLoopVulnerable;

#[contractimpl]
impl UnboundedLoopVulnerable {
    // VULN: iterates over caller-supplied data with no upper bound, so a large
    // input exhausts the instruction budget (out-of-gas / DoS).
    pub fn sum_all(_env: Env, items: Vec<i128>) -> i128 {
        let mut total: i128 = 0;
        for item in items.iter() {
            total = total.saturating_add(item);
        }
        total
    }
}
