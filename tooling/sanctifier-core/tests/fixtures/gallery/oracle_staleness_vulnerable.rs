#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol};

// GALLERY: oracle staleness
// Mapped finding: S006 (unsafe_pattern) — planned detector.

#[contract]
pub struct OracleStalenessVulnerable;

#[contractimpl]
impl OracleStalenessVulnerable {
    // VULN: returns the stored price without checking its age, so a stale or
    // expired price can be used for critical pricing/liquidation math.
    pub fn get_price(env: Env, asset: Symbol) -> i128 {
        let price: i128 = env.storage().persistent().get(&asset).unwrap_or(0);
        env.storage().persistent().extend_ttl(&asset, 100, 1000);
        price
    }
}
