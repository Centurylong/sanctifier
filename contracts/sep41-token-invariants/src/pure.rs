//! Pure arithmetic functions encoding SEP-41 token semantics.
//!
//! These functions contain no `soroban_sdk::Env` dependency so they can be
//! called by both the contract layer and by Kani proof harnesses without any
//! host-function FFI.
//!
//! ## SEP-41 Coverage
//!
//! | Operation       | Function                         | Invariants encoded       |
//! |-----------------|----------------------------------|--------------------------|
//! | `transfer`      | `transfer_pure`                  | Conservation, rejection  |
//! | `transfer_from` | `transfer_from_pure`             | Conservation, allowance  |
//! | `approve`       | `approve_pure`                   | Allowance consistency    |
//! | `burn`          | `burn_pure`                      | Supply reduction         |
//! | `mint`          | `mint_pure`                      | Supply increase          |

// ── SEP-41 Transfer ──────────────────────────────────────────────────────────

/// Apply a transfer between two balances.
/// Returns `(new_from, new_to)` or an error string if the amount is invalid
/// or would cause underflow/overflow.
///
/// This encodes the core SEP-41 requirement: `transfer` decreases `from`,
/// increases `to` by the same amount, and reverts on insufficient balance.
pub fn transfer_pure(from: i128, to: i128, amount: i128) -> Result<(i128, i128), &'static str> {
    if amount <= 0 {
        return Err("amount must be positive");
    }
    if from < amount {
        return Err("insufficient balance");
    }
    let new_from = from - amount;
    let new_to = to.checked_add(amount).ok_or("receiver overflow")?;
    Ok((new_from, new_to))
}

// ── SEP-41 Allowance / Approve ───────────────────────────────────────────────

/// Apply an approval. Returns the new allowance or an error.
///
/// Per SEP-41, `approve` sets the allowance to the given amount unconditionally.
/// The `live_until_ledger` parameter is a chain-level concern not captured in
/// pure logic (it lives in the Host layer).
pub fn approve_pure(amount: i128) -> Result<i128, &'static str> {
    if amount < 0 {
        return Err("approval amount cannot be negative");
    }
    Ok(amount)
}

/// Check allowance consistency after approve.
///
/// Returns `true` if the new allowance matches the approved amount.
pub fn allowance_consistent_after_approve(amount: i128) -> bool {
    matches!(approve_pure(amount), Ok(new) if new == amount)
}

// ── SEP-41 Transfer-From ─────────────────────────────────────────────────────

/// Transfer tokens using an allowance (spender pulls from owner).
/// Returns `(new_from, new_to, new_allowance)` or an error.
///
/// Per SEP-41: `transfer_from` decreases `from` balance, increases `to` balance,
/// and decrements the allowance by the transfer amount.
pub fn transfer_from_pure(
    from: i128,
    to: i128,
    allowance: i128,
    amount: i128,
) -> Result<(i128, i128, i128), &'static str> {
    if amount <= 0 {
        return Err("amount must be positive");
    }
    if allowance < amount {
        return Err("insufficient allowance");
    }
    if from < amount {
        return Err("insufficient balance");
    }
    let new_from = from - amount;
    let new_to = to.checked_add(amount).ok_or("receiver overflow")?;
    let new_allowance = allowance - amount;
    Ok((new_from, new_to, new_allowance))
}

// ── SEP-41 Mint / Burn ───────────────────────────────────────────────────────

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
    if balance < amount {
        return Err("insufficient balance to burn");
    }
    Ok(balance - amount)
}

// ── SEP-41 Invariants ────────────────────────────────────────────────────────

/// **Invariant: Supply conservation across transfers.**
///
/// After any `transfer`, the sum of `from + to` balances is unchanged.
/// Invalid transfers (bad amount, overflow) are treated as no-ops.
///
/// This is the primary SEP-41 conservation invariant.
pub fn supply_conserved_after_transfer(from: i128, to: i128, amount: i128) -> bool {
    let original_sum = from.checked_add(to);
    match transfer_pure(from, to, amount) {
        Ok((new_from, new_to)) => {
            let new_sum = new_from.checked_add(new_to);
            original_sum == new_sum
        }
        Err(_) => true, // invalid transfer is a no-op
    }
}

/// **Invariant: Supply conservation across transfer_from.**
///
/// After any `transfer_from`, the sum of `from + to` balances is unchanged,
/// and the allowance is correctly decremented.
pub fn supply_conserved_after_transfer_from(
    from: i128,
    to: i128,
    allowance: i128,
    amount: i128,
) -> bool {
    let original_sum = from.checked_add(to);
    match transfer_from_pure(from, to, allowance, amount) {
        Ok((new_from, new_to, new_allowance)) => {
            let sum_conserved = {
                let new_sum = new_from.checked_add(new_to);
                original_sum == new_sum
            };
            let allowance_decremented = new_allowance == allowance - amount;
            sum_conserved && allowance_decremented
        }
        Err(_) => true, // invalid operation is a no-op
    }
}

/// **Invariant: Allowance consistency after approve.**
///
/// After `approve`, the allowance must equal the approved amount.
pub fn allowance_is_set_by_approve(amount: i128) -> bool {
    allowance_consistent_after_approve(amount)
}

/// **Invariant: Burn reduces supply.**
///
/// After burning `amount` from `balance`, the result is exactly `balance - amount`.
pub fn burn_reduces_balance(balance: i128, amount: i128) -> bool {
    match burn_pure(balance, amount) {
        Ok(new) => new == balance - amount,
        Err(_) => true, // invalid burn is a no-op
    }
}

/// **Invariant: Mint increases supply.**
///
/// After minting `amount` into `balance`, the result is exactly `balance + amount`.
pub fn mint_increases_balance(balance: i128, amount: i128) -> bool {
    match mint_pure(balance, amount) {
        Ok(new) => new == balance + amount,
        Err(_) => true,
    }
}

/// **Invariant: Transfer rejects insufficient balance.**
///
/// When `from < amount`, transfer must fail.
pub fn transfer_rejects_insufficient_balance(from: i128, to: i128, amount: i128) -> bool {
    if from < amount && amount > 0 {
        transfer_pure(from, to, amount).is_err()
    } else {
        true
    }
}

/// **Invariant: Allowance is strictly enforced in transfer_from.**
///
/// When `allowance < amount`, transfer_from must fail.
pub fn transfer_from_rejects_insufficient_allowance(
    from: i128,
    to: i128,
    allowance: i128,
    amount: i128,
) -> bool {
    if allowance < amount && amount > 0 && from >= amount {
        transfer_from_pure(from, to, allowance, amount).is_err()
    } else {
        true
    }
}

// Pure function tests are in tests/pure_tests.rs to avoid the
// compile-time issue with env.register() in lib test binaries.
