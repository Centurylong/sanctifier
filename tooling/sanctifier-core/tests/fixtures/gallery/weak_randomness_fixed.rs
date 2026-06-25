#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// GALLERY: weak randomness (fixed)
// Uses the host PRNG instead of a predictable ledger value.

#[contract]
pub struct WeakRandomnessFixed;

#[contractimpl]
impl WeakRandomnessFixed {
    // FIX: draw from the Soroban host PRNG, which is not attacker-predictable.
    pub fn pick_winner(env: Env, players: u32) -> u32 {
        let rand = env.prng().u64_in_range(0..(players as u64));
        rand as u32
    }
}
