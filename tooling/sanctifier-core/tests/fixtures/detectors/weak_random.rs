#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

// FIXTURE: weak_random detector.
// Ledger metadata is predictable and must not choose lottery outcomes.

#[contract]
pub struct Lottery;

#[contractimpl]
impl Lottery {
    // Violation: predictable wall-clock metadata reduced into a player index.
    pub fn choose_winner(env: Env, players: Vec<Address>) -> Address {
        let winner_idx = (env.ledger().timestamp() % players.len() as u64) as u32;
        players.get(winner_idx).unwrap()
    }

    // Violation: predictable ledger sequence reduced into a player index.
    pub fn choose_backup(env: Env, players: Vec<Address>) -> Address {
        let idx = env.ledger().sequence() % players.len();
        players[idx]
    }

    // Safe: ledger time is used only for a deadline, not for selection.
    pub fn deadline(env: Env) -> u64 {
        env.ledger().timestamp() + 60
    }

    // Safe: selection comes from a user reveal / commitment-derived value.
    pub fn choose_revealed(players: Vec<Address>, reveal: BytesN<32>) -> Address {
        let idx = hash_to_index(reveal) % players.len();
        players.get(idx).unwrap()
    }
}
