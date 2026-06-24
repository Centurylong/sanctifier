#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol};

// GALLERY: oracle staleness (fixed)
// Rejects prices older than a maximum age before returning them.

#[contract]
pub struct OracleStalenessFixed;

#[contractimpl]
impl OracleStalenessFixed {
    // FIX: store (price, updated_at) and refuse to serve a price that is too old.
    pub fn get_price(env: Env, asset: Symbol) -> i128 {
        const MAX_AGE: u64 = 300;
        let (price, updated_at): (i128, u64) =
            env.storage().persistent().get(&asset).unwrap_or((0, 0));
        env.storage().persistent().extend_ttl(&asset, 100, 1000);
        let age = env.ledger().timestamp().saturating_sub(updated_at);
        if age > MAX_AGE {
            return 0;
        }
        price
    }
}
