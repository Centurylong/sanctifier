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
    pub fn pick_winner(env: Env, players: u32) -> u32 {
        let seed = env.ledger().timestamp();
        (seed as u32) % players
    }
}
