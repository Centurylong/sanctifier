#![no_std]

// FIXTURE: signed_nonnegative_quantity detector
// Signed balance/amount values are advisory findings unless their domain is guarded.

struct VaultState {
    balance: i128,
    balance_delta: i128,
    reserve_amount: u128,
}

fn withdraw(amount: i128) {
    consume(amount);
}

fn guarded_credit(balance: i64) {
    assert!(balance >= 0);
    consume(balance);
}
