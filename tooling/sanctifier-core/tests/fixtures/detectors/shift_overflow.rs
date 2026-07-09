#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: shift_overflow detector
// Runtime shift amounts must be bounded below the left operand width.

#[contract]
pub struct ShiftContract;

#[contractimpl]
impl ShiftContract {
    // Violation: runtime shift amount may be >= 64.
    pub fn variable_left_shift(_env: Env, mask: u64, shift: u32) -> u64 {
        mask << shift
    }

    // Violation: literal shift equals the u32 bit width.
    pub fn literal_right_shift(_env: Env, mask: u32) -> u32 {
        mask >> 32
    }

    // Clean: prior guard bounds the shift amount.
    pub fn guarded_shift(_env: Env, mask: u64, shift: u32) -> u64 {
        if shift >= u64::BITS {
            panic!("bad shift");
        }
        mask << shift
    }

    // Clean: non-zero literal below the left operand width.
    pub fn safe_literal_shift(_env: Env, mask: u64) -> u64 {
        mask << 8
    }
}
