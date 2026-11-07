//! Kani proof harnesses for the token-invariants contract.
//!
//! These harnesses formally verify the pure-logic functions in `pure.rs`.
//! They follow the same pattern established in `contracts/kani-poc`:
//! extract pure arithmetic, leave the Soroban host layer unverified.
//!
//! Run with:
//! ```sh
//! cargo kani --package token-invariants
//! ```

#[cfg(kani)]
mod proofs {
    use crate::pure::*;

    /// **Property**: Every valid transfer conserves the total of
    /// `from + to` balances — no tokens are created or destroyed.
    #[kani::proof]
    fn verify_transfer_conserves_supply() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let amount: i128 = kani::any();

        kani::assume(amount > 0);
        kani::assume(from >= amount);
        kani::assume(from <= i128::MAX);
        kani::assume(to >= 0);
        kani::assume(to <= i128::MAX - amount);
        // Avoid overflow when computing from + to
        kani::assume(from <= i128::MAX - to);

        assert!(
            supply_is_conserved_after_transfer(from, to, amount),
            "supply conservation invariant violated"
        );
    }

    /// **Property**: `transfer_pure` always fails when `amount <= 0`.
    #[kani::proof]
    fn verify_transfer_rejects_non_positive_amount() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount <= 0);
        assert!(transfer_pure(from, to, amount).is_err());
    }

    /// **Property**: `transfer_pure` fails on sender underflow.
    #[kani::proof]
    fn verify_transfer_rejects_underflow() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount > 0);
        kani::assume(from < amount);
        assert!(transfer_pure(from, to, amount).is_err());
    }

    /// **Property**: `mint_pure` fails when `amount <= 0`.
    #[kani::proof]
    fn verify_mint_rejects_non_positive() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount <= 0);
        assert!(mint_pure(balance, amount).is_err());
    }

    /// **Property**: `mint_pure` produces `balance + amount` for valid inputs.
    #[kani::proof]
    fn verify_mint_correct_result() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount > 0);
        kani::assume(balance >= 0);
        kani::assume(balance <= i128::MAX - amount);
        let new = mint_pure(balance, amount).expect("mint should succeed");
        assert_eq!(new, balance + amount);
    }

    /// **Property**: `burn_pure` fails when balance is insufficient.
    #[kani::proof]
    fn verify_burn_rejects_insufficient_balance() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount > 0);
        kani::assume(balance < amount);
        assert!(burn_pure(balance, amount).is_err());
    }

    /// **Property**: Token transfer requires caller authorization.
    /// An unauthorized caller (caller != owner) is strictly rejected,
    /// ensuring no unauthorized transfer or balance debit can ever occur.
    #[kani::proof]
    fn verify_transfer_rejects_unauthorized_caller() {
        let from_balance: i128 = kani::any();
        let to_balance: i128 = kani::any();
        let amount: i128 = kani::any();
        let caller_id: u32 = kani::any();
        let owner_id: u32 = kani::any();

        kani::assume(caller_id != owner_id);

        let result = transfer_authorized_pure(from_balance, to_balance, amount, caller_id, owner_id);
        assert!(result.is_err(), "Unauthorized transfer must be rejected");
        assert_eq!(result.unwrap_err(), "unauthorized transfer");
    }

    /// **Property**: Authorized caller (caller == owner) with valid bounds succeeds
    /// and conserves supply.
    #[kani::proof]
    fn verify_authorized_transfer_conserves_supply() {
        let from_balance: i128 = kani::any();
        let to_balance: i128 = kani::any();
        let amount: i128 = kani::any();
        let caller_id: u32 = kani::any();
        let owner_id: u32 = caller_id; // Authorized

        kani::assume(amount > 0);
        kani::assume(from_balance >= amount);
        kani::assume(from_balance <= i128::MAX);
        kani::assume(to_balance >= 0);
        kani::assume(to_balance <= i128::MAX - amount);
        kani::assume(from_balance <= i128::MAX - to_balance);

        let result = transfer_authorized_pure(from_balance, to_balance, amount, caller_id, owner_id);
        assert!(result.is_ok(), "Authorized transfer with valid bounds must succeed");
        let (new_from, new_to) = result.unwrap();
        assert_eq!(from_balance + to_balance, new_from + new_to);
    }
}

