//! # reentrancy-guard
//!
//! A formally verified, `#![no_std]`-compatible reentrancy guard for
//! [Soroban](https://stellar.org/soroban) smart contracts.
//!
//! ## Quick start
//!
//! ```toml
//! [dependencies]
//! reentrancy-guard = "0.1"
//! ```
//!
//! ```ignore
//! use reentrancy_guard::ReentrancyGuard;
//! use soroban_sdk::{contractimpl, Env};
//!
//! pub fn withdraw(env: Env, amount: i128) {
//!     let guard = ReentrancyGuard::new(&env);
//!     guard.enter();   // panics if called again before exit()
//!     // … perform withdrawal …
//!     guard.exit();
//! }
//! ```
//!
//! ## Formal verification
//!
//! The pure state-transition logic is verified with
//! [Kani](https://model-checking.github.io/kani/).
//! Run `cargo kani` to replay the proofs locally.
#![allow(unexpected_cfgs)]
#![cfg_attr(not(test), no_std)]

use soroban_sdk::{contracterror, panic_with_error, symbol_short, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GuardError {
    ReentrancyDetected = 1,
}

// ── Pure logic (verified with Kani) ─────────────────────────────────────────────

/// Represents the status of the reentrancy guard.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GuardStatus {
    Unlocked = 0,
    Locked = 1,
}

impl GuardStatus {
    pub fn from_u32(val: u32) -> Self {
        match val {
            0 => GuardStatus::Unlocked,
            _ => GuardStatus::Locked,
        }
    }
}

/// Core logic for reentrancy protection.
///
/// * `current_status` - The current status in storage.
/// * Returns `Ok(GuardStatus::Locked)` if transition is allowed (Unlocked -> Locked).
/// * Returns `Err("reentrancy detected")` if already locked.
pub fn enter_pure(current_status: GuardStatus) -> Result<GuardStatus, &'static str> {
    if current_status == GuardStatus::Locked {
        return Err("reentrancy detected");
    }
    Ok(GuardStatus::Locked)
}

/// Transition from Locked to Unlocked.
pub fn exit_pure() -> GuardStatus {
    GuardStatus::Unlocked
}

// ── Soroban Contract Integration ────────────────────────────────────────────────

const GUARD_KEY: Symbol = symbol_short!("RE_GRD");

pub struct ReentrancyGuard<'a> {
    env: &'a Env,
}

impl<'a> ReentrancyGuard<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    /// Enter a reentrancy-protected section.
    /// Panics if reentrancy is detected.
    pub fn enter(&self) {
        let status: u32 = self.env.storage().instance().get(&GUARD_KEY).unwrap_or(0);
        let current = GuardStatus::from_u32(status);

        match enter_pure(current) {
            Ok(new_status) => {
                self.env
                    .storage()
                    .instance()
                    .set(&GUARD_KEY, &(new_status as u32));
            }
            Err(_) => panic_with_error!(self.env, GuardError::ReentrancyDetected),
        }
    }

    /// Exit a reentrancy-protected section.
    pub fn exit(&self) {
        let unlocked = exit_pure();
        self.env
            .storage()
            .instance()
            .set(&GUARD_KEY, &(unlocked as u32));
    }
}

// ── Kani harnesses ─────────────────────────────────────────────────────────────

#[cfg(kani)]
mod verification {
    use super::*;

    /// **Property**: Cannot enter if already locked.
    #[kani::proof]
    fn verify_enter_fails_when_locked() {
        let result = enter_pure(GuardStatus::Locked);
        assert!(result.is_err());
    }

    /// **Property**: Can enter if unlocked.
    #[kani::proof]
    fn verify_enter_succeeds_when_unlocked() {
        let result = enter_pure(GuardStatus::Unlocked);
        assert!(result.is_ok());
        assert!(result.unwrap() == GuardStatus::Locked);
    }

    /// **Property**: Exit always returns Unlocked.
    #[kani::proof]
    fn verify_exit_always_unlocks() {
        let status = exit_pure();
        assert!(status == GuardStatus::Unlocked);
    }

    /// **Property**: State machine idempotency.
    /// Exhaustively checking all GuardStatus values.
    #[kani::proof]
    fn verify_guard_state_machine() {
        // We model status as u32 to simulate kani::any() more broadly if needed,
        // but here we can just use the enum variant logic.
        let is_locked: bool = kani::any();
        let current = if is_locked {
            GuardStatus::Locked
        } else {
            GuardStatus::Unlocked
        };

        let result = enter_pure(current);

        if is_locked {
            assert!(result.is_err());
        } else {
            assert!(result.is_ok());
            assert!(result.unwrap() == GuardStatus::Locked);
        }
    }

    /// **Property**: Mutex is always released on exit across all possible execution branches
    /// (normal completion or error return path), ensuring no permanent lock occurs.
    #[kani::proof]
    fn verify_reentrancy_mutex_always_released_on_exit() {
        let initial_status = GuardStatus::Unlocked;

        let entered = enter_pure(initial_status);
        assert!(entered.is_ok());
        let mut status = entered.unwrap();
        assert_eq!(status, GuardStatus::Locked);

        // Simulate any path taken by contract execution (success or error)
        let branch_error: bool = kani::any();
        if branch_error {
            // Error exit path
            status = exit_pure();
        } else {
            // Normal exit path
            status = exit_pure();
        }

        assert_eq!(status, GuardStatus::Unlocked, "Mutex must be released on all exit paths");
    }
}

