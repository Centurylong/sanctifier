//! Kani proof harnesses for SEP-41 token invariants.
//!
//! These harnesses formally verify the pure-logic functions in `pure.rs`.
//! They follow the same pattern established in `contracts/kani-poc` and
//! `contracts/token-invariants`: extract pure arithmetic, leave the Soroban
//! host layer unverified.
//!
//! ## Running proofs
//!
//! ```sh
//! cargo kani --package sep41-token-invariants
//! ```
//!
//! ## Invariants proven
//!
//! | Proof                                          | SEP-41 requirement           |
//! |------------------------------------------------|------------------------------|
//! | `verify_transfer_conserves_supply`             | Conservation of total supply |
//! | `verify_transfer_rejects_non_positive`         | Reverts on amount ≤ 0        |
//! | `verify_transfer_rejects_insufficient_balance` | Reverts on underflow         |
//! | `verify_transfer_from_conserves_supply`        | Supply + allowance invariant |
//! | `verify_approve_sets_allowance`                | Allowance consistency        |
//! | `verify_burn_reduces_balance`                  | Burn reduces supply          |
//! | `verify_mint_increases_balance`                | Mint increases supply        |
//! | `verify_burn_rejects_insufficient`             | Reverts on underflow         |
//! | `verify_transfer_from_rejects_low_allowance`   | Allowance enforcement        |

#[cfg(kani)]
mod proofs {
    use crate::pure::*;

    // ── Transfer invariants ────────────────────────────────────────────────

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
        kani::assume(from <= i128::MAX - to);

        assert!(
            supply_conserved_after_transfer(from, to, amount),
            "SEP-41: supply conservation invariant violated"
        );
    }

    /// **Property**: `transfer_pure` always fails when `amount <= 0`.
    #[kani::proof]
    fn verify_transfer_rejects_non_positive() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount <= 0);
        assert!(
            transfer_pure(from, to, amount).is_err(),
            "SEP-41: transfer must revert for non-positive amount"
        );
    }

    /// **Property**: `transfer_pure` fails on sender underflow
    /// (insufficient balance).
    #[kani::proof]
    fn verify_transfer_rejects_insufficient_balance() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount > 0);
        kani::assume(from < amount);
        assert!(
            transfer_pure(from, to, amount).is_err(),
            "SEP-41: transfer must revert on insufficient balance"
        );
    }

    // ── Transfer-From invariants ───────────────────────────────────────────

    /// **Property**: Every valid `transfer_from` conserves the sum of
    /// `from + to` balances AND correctly decrements the allowance.
    #[kani::proof]
    fn verify_transfer_from_conserves_supply() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let allowance: i128 = kani::any();
        let amount: i128 = kani::any();

        kani::assume(amount > 0);
        kani::assume(from >= amount);
        kani::assume(allowance >= amount);
        kani::assume(from <= i128::MAX);
        kani::assume(to >= 0);
        kani::assume(to <= i128::MAX - amount);
        kani::assume(from <= i128::MAX - to);

        assert!(
            supply_conserved_after_transfer_from(from, to, allowance, amount),
            "SEP-41: transfer_from conservation + allowance invariant violated"
        );
    }

    /// **Property**: `transfer_from_pure` fails when allowance is
    /// insufficient.
    #[kani::proof]
    fn verify_transfer_from_rejects_low_allowance() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let allowance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount > 0);
        kani::assume(from >= amount);
        kani::assume(allowance < amount);
        assert!(
            transfer_from_pure(from, to, allowance, amount).is_err(),
            "SEP-41: transfer_from must revert on insufficient allowance"
        );
    }

    // ── Approve invariants ─────────────────────────────────────────────────

    /// **Property**: `approve` sets the allowance to the given amount.
    #[kani::proof]
    fn verify_approve_sets_allowance() {
        let amount: i128 = kani::any();
        kani::assume(amount >= 0);
        let new = approve_pure(amount).expect("approve should succeed for non-negative");
        assert_eq!(
            new, amount,
            "SEP-41: approve must set allowance to the approved amount"
        );
    }

    /// **Property**: `approve_pure` fails on negative amounts.
    #[kani::proof]
    fn verify_approve_rejects_negative() {
        let amount: i128 = kani::any();
        kani::assume(amount < 0);
        assert!(
            approve_pure(amount).is_err(),
            "SEP-41: approve must reject negative amounts"
        );
    }

    // ── Mint / Burn invariants ─────────────────────────────────────────────

    /// **Property**: `mint_pure` produces `balance + amount` for valid inputs.
    #[kani::proof]
    fn verify_mint_increases_balance() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount > 0);
        kani::assume(balance >= 0);
        kani::assume(balance <= i128::MAX - amount);
        let new = mint_pure(balance, amount).expect("SEP-41: mint should succeed");
        assert_eq!(new, balance + amount, "SEP-41: mint must increase balance by amount");
    }

    /// **Property**: `mint_pure` fails when `amount <= 0`.
    #[kani::proof]
    fn verify_mint_rejects_non_positive() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount <= 0);
        assert!(
            mint_pure(balance, amount).is_err(),
            "SEP-41: mint must revert for non-positive amount"
        );
    }

    /// **Property**: `burn_pure` produces `balance - amount` for valid inputs.
    #[kani::proof]
    fn verify_burn_reduces_balance() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount > 0);
        kani::assume(balance >= amount);
        kani::assume(balance <= i128::MAX);
        let new = burn_pure(balance, amount).expect("SEP-41: burn should succeed");
        assert_eq!(new, balance - amount, "SEP-41: burn must reduce balance by amount");
    }

    /// **Property**: `burn_pure` fails when balance is insufficient.
    #[kani::proof]
    fn verify_burn_rejects_insufficient() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount > 0);
        kani::assume(balance < amount);
        assert!(
            burn_pure(balance, amount).is_err(),
            "SEP-41: burn must revert on insufficient balance"
        );
    }

    /// **Property**: `burn_pure` fails when `amount <= 0`.
    #[kani::proof]
    fn verify_burn_rejects_non_positive() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();
        kani::assume(amount <= 0);
        assert!(
            burn_pure(balance, amount).is_err(),
            "SEP-41: burn must revert for non-positive amount"
        );
    }

    // ── Composite invariant ────────────────────────────────────────────────

    /// **Property**: The `transfer_rejects_insufficient_balance` invariant
    /// holds for all `(from, to, amount)` triples.
    #[kani::proof]
    fn verify_transfer_rejects_insufficient_invariant() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let amount: i128 = kani::any();
        assert!(
            transfer_rejects_insufficient_balance(from, to, amount),
            "SEP-41: invariant: transfer must revert when from < amount"
        );
    }

    /// **Property**: The `transfer_from_rejects_insufficient_allowance`
    /// invariant holds for all parameter combinations.
    #[kani::proof]
    fn verify_transfer_from_allowance_invariant() {
        let from: i128 = kani::any();
        let to: i128 = kani::any();
        let allowance: i128 = kani::any();
        let amount: i128 = kani::any();
        assert!(
            transfer_from_rejects_insufficient_allowance(from, to, allowance, amount),
            "SEP-41: invariant: transfer_from must revert when allowance < amount"
        );
    }
}
