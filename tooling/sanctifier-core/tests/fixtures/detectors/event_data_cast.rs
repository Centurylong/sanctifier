#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

const TOPIC: Symbol = symbol_short!("TOPIC");

// FIXTURE: event_data_cast detector
// Narrowing integer casts inside event emission data silently truncate
// values that indexers consume. This detector flags casts where the
// target type is narrower (fewer bits) or has different signedness.

#[contract]
pub struct CastContract;

#[contractimpl]
impl CastContract {
    // VIOLATION: i128 → u32 loses 96 bits and signedness.
    pub fn deposit(env: Env, amount: i128) {
        env.events().publish((TOPIC,), (amount as u32,));
    }

    // VIOLATION: i64 → u64 keeps width but loses sign information.
    pub fn wrap_signed(env: Env, amount: i64) {
        env.events().publish((TOPIC,), (amount as u64,));
    }

    // VIOLATION: two narrowing casts in one event.
    pub fn swap(env: Env, amount_in: i128, amount_out: i64) {
        env.events().publish(
            (TOPIC,),
            (amount_in as u64, amount_out as u32),
        );
    }

    // SAFE: widening cast (u32 → u64). No information loss.
    pub fn widen(env: Env, amount: u32) {
        env.events().publish((TOPIC,), (amount as u64,));
    }

    // SAFE: no cast at all — value emitted with full width.
    pub fn raw(env: Env, amount: i128) {
        env.events().publish((TOPIC,), (amount,));
    }

    // SAFE: cast outside event context. Not in scope for this detector.
    pub fn truncate_elsewhere(_env: Env, amount: i128) -> u32 {
        amount as u32
    }
}
