#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: zero_denominator detector
// Division and modulo by unchecked variables can panic on-chain.

#[contract]
pub struct RatioContract;

#[contractimpl]
impl RatioContract {
    // Violation: supply may be zero.
    pub fn price_per_share(_env: Env, total_assets: i128, supply: i128) -> i128 {
        total_assets / supply
    }

    // Violation: epoch_length may be zero.
    pub fn epoch_offset(_env: Env, ledger: u64, epoch_length: u64) -> u64 {
        ledger % epoch_length
    }

    // Clean: prior guard proves the denominator is non-zero.
    pub fn guarded_price(_env: Env, total_assets: i128, supply: i128) -> i128 {
        if supply == 0 {
            panic!("zero supply");
        }
        total_assets / supply
    }

    // Clean: assert-style guard proves the denominator is non-zero.
    pub fn guarded_epoch(_env: Env, ledger: u64, epoch_length: u64) -> u64 {
        assert!(epoch_length > 0);
        ledger % epoch_length
    }

    // Clean: a non-zero literal denominator cannot be zero.
    pub fn half(_env: Env, total_assets: i128) -> i128 {
        total_assets / 2
    }
}
