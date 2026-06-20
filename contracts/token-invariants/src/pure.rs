//! Pure arithmetic functions for the token contract.
//!
//! These functions contain no `soroban_sdk::Env` dependency so they can be
//! called by both the contract layer and by Kani proof harnesses without any
//! host-function FFI.

/// Apply a transfer between two balances. Returns `(new_from, new_to)` or
/// an error string if the amount is invalid or would cause underflow/overflow.
pub fn transfer_pure(from: i128, to: i128, amount: i128) -> Result<(i128, i128), &'static str> {
    if amount <= 0 {
        return Err("amount must be positive");
    }
    let new_from = from.checked_sub(amount).ok_or("insufficient balance")?;
    let new_to = to.checked_add(amount).ok_or("receiver overflow")?;
    Ok((new_from, new_to))
}

/// Mint `amount` tokens into `balance`.
pub fn mint_pure(balance: i128, amount: i128) -> Result<i128, &'static str> {
    if amount <= 0 {
        return Err("mint amount must be positive");
    }
    balance.checked_add(amount).ok_or("mint overflow")
}

/// Burn `amount` tokens from `balance`.
pub fn burn_pure(balance: i128, amount: i128) -> Result<i128, &'static str> {
    if amount <= 0 {
        return Err("burn amount must be positive");
    }
    balance
        .checked_sub(amount)
        .ok_or("insufficient balance to burn")
}

/// Verify the core supply invariant in pure arithmetic:
/// after any transfer, `from + to` equals the original `from + to`.
/// This is the property `sanctifier verify` will report on and Kani will prove.
///
/// Returns `true` when the invariant holds. Invalid transfers (bad amount, overflow)
/// are treated as no-ops so the invariant trivially holds for those inputs.
///
/// # Examples
/// ```
/// use token_invariants::pure::supply_is_conserved_after_transfer;
/// assert!(supply_is_conserved_after_transfer(100, 50, 25));
/// assert!(supply_is_conserved_after_transfer(100, 50, 0)); // invalid → no-op
/// ```
pub fn supply_is_conserved_after_transfer(from: i128, to: i128, amount: i128) -> bool {
    let original_sum = from.checked_add(to);
    match transfer_pure(from, to, amount) {
        Ok((new_from, new_to)) => {
            let new_sum = new_from.checked_add(new_to);
            original_sum == new_sum
        }
        Err(_) => true, // invalid transfer is a no-op; invariant trivially holds
    }
}
