#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Vec};

// GALLERY: unbounded loop (fixed)
// Caps the input length so the work is bounded by a constant.

#[contract]
pub struct UnboundedLoopFixed;

#[contractimpl]
impl UnboundedLoopFixed {
    // FIX: reject oversized inputs before iterating.
    pub fn sum_all(_env: Env, items: Vec<i128>) -> i128 {
        const MAX_ITEMS: u32 = 100;
        if items.len() > MAX_ITEMS {
            return 0;
        }
        let mut total: i128 = 0;
        for item in items.iter() {
            total = total.saturating_add(item);
        }
        total
    }
}
