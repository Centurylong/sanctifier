#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// GALLERY: weak randomness
// Mapped finding: S006 (unsafe_pattern) — planned detector.

#[contract]
pub struct WeakRandomnessVulnerable;

#[contractimpl]
impl WeakRandomnessVulnerable {
    // VULN: derives "randomness" from the ledger timestamp, which validators
    // can observe/influence — the outcome is predictable and gameable.
    //
    // (The `players == 0` guard below is unrelated to this gallery entry: it
    // only exists so this fixture stays a clean, single-issue example for the
    // weak-randomness class instead of also tripping S018/division_by_zero.)
    pub fn pick_winner(env: Env, players: u32) -> u32 {
        if players == 0 {
            return 0;
        }
        let seed = env.ledger().timestamp();
        (seed as u32) % players
    }
}
