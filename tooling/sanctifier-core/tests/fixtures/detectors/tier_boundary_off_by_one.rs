#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: tier_boundary_off_by_one detector
// An if/else-if boundary ladder that mixes strict (<, >) and inclusive
// (<=, >=) comparisons against the same variable is a classic off-by-one:
// the shared boundary value either double-matches (misassigned to the
// earlier branch) or matches neither (falls through unintended).

#[contract]
pub struct TierBoundaryContract;

#[contractimpl]
impl TierBoundaryContract {
    // Violation: `score < 50` then `score <= 80` — a score of exactly 80
    // matches the Silver branch as intended, but the *inconsistency* itself
    // (mixing < and <=) is the signal; here it also means a score of exactly
    // 50 falls into Silver instead of a clearly-defined Bronze/Silver split.
    pub fn tier_of(env: Env, score: u32) -> u32 {
        if score < 50 {
            0 // Bronze
        } else if score <= 80 {
            1 // Silver
        } else {
            2 // Gold
        }
    }

    // Violation: descending ladder mixing `>` then `>=`.
    pub fn rank_of(env: Env, points: i128) -> u32 {
        if points > 1000 {
            0 // Diamond
        } else if points >= 500 {
            1 // Platinum
        } else {
            2 // Standard
        }
    }

    // Safe: consistent `<` throughout — each value belongs to exactly one tier.
    pub fn tier_of_consistent(env: Env, score: u32) -> u32 {
        if score < 50 {
            0
        } else if score < 80 {
            1
        } else {
            2
        }
    }

    // Safe: consistent `<=` throughout.
    pub fn tier_of_inclusive(env: Env, score: u32) -> u32 {
        if score <= 49 {
            0
        } else if score <= 79 {
            1
        } else {
            2
        }
    }

    // Safe: single-branch if, no ladder to be inconsistent within.
    pub fn is_eligible(env: Env, score: u32) -> bool {
        if score < 50 {
            return false;
        }
        true
    }
}
