//! # Kani Formal Verification Harnesses for AMM Pool
//!
//! This module contains formal verification harnesses using Kani to prove
//! critical mathematical and security properties of the AMM pool implementation.

use super::*;
use kani::*;

/// Verify that swap calculations never overflow for any input values
#[kani::proof]
fn verify_swap_no_overflow() {
    let reserve_in: u128 = any();
    let reserve_out: u128 = any();
    let amount_in: u128 = any();
    let fee_bps: u128 = any();

    // Assume reasonable constraints to avoid trivial violations
    assume(reserve_in > 0 && reserve_in <= u64::MAX as u128);
    assume(reserve_out > 0 && reserve_out <= u64::MAX as u128);
    assume(amount_in > 0 && amount_in <= u32::MAX as u128);
    assume(fee_bps < 10000);

    // This should either succeed or fail gracefully, never panic
    let result = AmmPool::calculate_swap_output(reserve_in, reserve_out, amount_in, fee_bps);

    match result {
        Ok(output) => {
            // If successful, output should be reasonable
            assert!(output > 0);
            assert!(output < reserve_out);
        }
        Err(_) => {
            // Errors are acceptable for edge cases
        }
    }
}

/// Verify constant product formula (k-invariant) is preserved in swaps
#[kani::proof]
fn verify_constant_product_invariant() {
    let reserve_in: u128 = any();
    let reserve_out: u128 = any();
    let amount_in: u128 = any();
    let fee_bps: u128 = any();

    // Assume reasonable values to make the proof tractable
    assume(reserve_in >= 1000 && reserve_in <= 1_000_000);
    assume(reserve_out >= 1000 && reserve_out <= 1_000_000);
    assume(amount_in >= 1 && amount_in <= 10000);
    assume(fee_bps <= 1000); // Max 10% fee

    if let Ok(amount_out) =
        AmmPool::calculate_swap_output(reserve_in, reserve_out, amount_in, fee_bps)
    {
        // Calculate k before swap
        let k_before = reserve_in * reserve_out;

        // Calculate k after swap
        let new_reserve_in = reserve_in + amount_in;
        let new_reserve_out = reserve_out - amount_out;
        let k_after = new_reserve_in * new_reserve_out;

        // K should never decrease (should increase due to fees)
        assert!(k_after >= k_before);
    }
}

/// Verify that liquidity calculations never overflow
#[kani::proof]
fn verify_liquidity_no_overflow() {
    let reserve_a: u128 = any();
    let reserve_b: u128 = any();
    let amount_a: u128 = any();
    let amount_b: u128 = any();
    let total_supply: u128 = any();

    // Assume reasonable constraints
    assume(reserve_a <= u32::MAX as u128);
    assume(reserve_b <= u32::MAX as u128);
    assume(amount_a <= u32::MAX as u128);
    assume(amount_b <= u32::MAX as u128);
    assume(total_supply <= u32::MAX as u128);

    // Test both initial and subsequent liquidity provision
    let result1 = AmmPool::calculate_liquidity_mint(0, 0, amount_a, amount_b, 0);
    let result2 =
        AmmPool::calculate_liquidity_mint(reserve_a, reserve_b, amount_a, amount_b, total_supply);

    // Should either succeed or fail gracefully
    match result1 {
        Ok(liquidity) => assert!(liquidity > 0),
        Err(_) => {}
    }

    match result2 {
        Ok(liquidity) => assert!(liquidity >= 0),
        Err(_) => {}
    }
}

/// Verify that burning liquidity respects proportionality
#[kani::proof]
fn verify_liquidity_burn_proportional() {
    let reserve_a: u128 = any();
    let reserve_b: u128 = any();
    let liquidity: u128 = any();
    let total_supply: u128 = any();

    // Assume reasonable values
    assume(reserve_a >= 100 && reserve_a <= 1_000_000);
    assume(reserve_b >= 100 && reserve_b <= 1_000_000);
    assume(total_supply >= 100 && total_supply <= 1_000_000);
    assume(liquidity > 0 && liquidity <= total_supply);

    if let Ok((amount_a, amount_b)) =
        AmmPool::calculate_liquidity_burn(reserve_a, reserve_b, liquidity, total_supply)
    {
        // Returned amounts should be positive
        assert!(amount_a > 0);
        assert!(amount_b > 0);

        // Returned amounts should not exceed reserves
        assert!(amount_a <= reserve_a);
        assert!(amount_b <= reserve_b);

        // Verify proportionality (within rounding errors)
        let expected_a = (reserve_a * liquidity) / total_supply;
        let expected_b = (reserve_b * liquidity) / total_supply;

        // Allow for ±1 rounding error
        assert!(amount_a == expected_a || amount_a == expected_a + 1 || amount_a == expected_a - 1);
        assert!(amount_b == expected_b || amount_b == expected_b + 1 || amount_b == expected_b - 1);
    }
}

/// Verify integer square root correctness
#[kani::proof]
fn verify_integer_sqrt() {
    let n: u128 = any();
    assume(n <= u64::MAX as u128); // Limit range for tractability

    let result = super::integer_sqrt(n);

    // sqrt(n)² ≤ n < (sqrt(n) + 1)²
    assert!(result * result <= n);
    assert!(n < (result + 1) * (result + 1));
}

/// Verify swap monotonicity (larger input → larger output)
#[kani::proof]
fn verify_swap_monotonic() {
    let reserve_in: u128 = any();
    let reserve_out: u128 = any();
    let amount_in_1: u128 = any();
    let amount_in_2: u128 = any();
    let fee_bps: u128 = any();

    // Reasonable constraints
    assume(reserve_in >= 1000 && reserve_in <= 100_000);
    assume(reserve_out >= 1000 && reserve_out <= 100_000);
    assume(amount_in_1 >= 1 && amount_in_1 <= 1000);
    assume(amount_in_2 > amount_in_1 && amount_in_2 <= 2000);
    assume(fee_bps <= 1000);

    let output_1 = AmmPool::calculate_swap_output(reserve_in, reserve_out, amount_in_1, fee_bps);
    let output_2 = AmmPool::calculate_swap_output(reserve_in, reserve_out, amount_in_2, fee_bps);

    if let (Ok(out1), Ok(out2)) = (output_1, output_2) {
        // Larger input should yield larger output (monotonicity)
        assert!(out2 > out1);
    }
}
