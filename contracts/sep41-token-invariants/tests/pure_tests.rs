//! Unit tests for SEP-41 pure functions and invariants.
//!
//! These test the pure arithmetic functions in `sep41_token_invariants::pure`
//! without requiring `env.register()` or any Soroban host types.

use sep41_token_invariants::pure::*;

#[test]
fn test_transfer_conserves() {
    assert!(supply_conserved_after_transfer(100, 50, 25));
    assert!(supply_conserved_after_transfer(100, 50, 0)); // invalid → no-op
    assert!(supply_conserved_after_transfer(0, 0, 1)); // insufficient → no-op
}

#[test]
fn test_transfer_from_conserves() {
    assert!(supply_conserved_after_transfer_from(100, 50, 60, 25));
    assert!(supply_conserved_after_transfer_from(100, 50, 60, 150)); // insufficient allowance → no-op
}

#[test]
fn test_approve_consistency() {
    assert!(allowance_is_set_by_approve(500));
    assert!(allowance_is_set_by_approve(0));
    assert!(!allowance_is_set_by_approve(-1)); // negative always fails Err
}

#[test]
fn test_mint_burn() {
    assert_eq!(mint_pure(100, 50).unwrap(), 150);
    assert_eq!(burn_pure(100, 50).unwrap(), 50);
    assert!(burn_pure(100, 150).is_err());
}

#[test]
fn test_mint_increases_balance() {
    assert!(mint_increases_balance(100, 50));
}

#[test]
fn test_burn_reduces_balance() {
    assert!(burn_reduces_balance(100, 50));
}

#[test]
fn test_transfer_rejects_insufficient() {
    assert!(transfer_rejects_insufficient_balance(10, 0, 20));
    assert!(transfer_rejects_insufficient_balance(20, 0, 10)); // sufficient → invariant holds trivially
}

#[test]
fn test_transfer_from_rejects_insufficient_allowance() {
    assert!(transfer_from_rejects_insufficient_allowance(100, 0, 10, 50));
    assert!(transfer_from_rejects_insufficient_allowance(100, 0, 60, 50)); // sufficient → invariant holds trivially
}

#[test]
fn test_transfer_result() {
    let result = transfer_pure(100, 0, 50).unwrap();
    assert_eq!(result.0, 50); // from decreased
    assert_eq!(result.1, 50); // to increased
}

#[test]
fn test_transfer_from_result() {
    let result = transfer_from_pure(100, 0, 60, 25).unwrap();
    assert_eq!(result.0, 75); // from decreased
    assert_eq!(result.1, 25); // to increased
    assert_eq!(result.2, 35); // allowance decreased
}

#[test]
fn test_mint_increases_balance_inv() {
    assert!(mint_increases_balance(100, 50));
    assert!(mint_increases_balance(100, 0)); // invalid → no-op (true)
}

#[test]
fn test_burn_reduces_balance_inv() {
    assert!(burn_reduces_balance(100, 50));
    assert!(burn_reduces_balance(100, 150)); // insufficient → no-op (true)
}

#[test]
fn test_approve_sets_allowance() {
    assert_eq!(approve_pure(500).unwrap(), 500);
    assert_eq!(approve_pure(0).unwrap(), 0);
    assert!(approve_pure(-1).is_err()); // negative should fail
}
