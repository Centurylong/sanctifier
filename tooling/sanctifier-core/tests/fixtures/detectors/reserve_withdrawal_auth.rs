#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

// FIXTURE: reserve_withdrawal_auth detector
// Public reserve/treasury withdrawals must include both admin authorization and
// nonce/replay protection before moving pooled funds.

#[contract]
pub struct ReserveTreasuryContract;

#[contractimpl]
impl ReserveTreasuryContract {
    // Violation: reserve transfer has no admin signer and no nonce guard.
    pub fn withdraw_reserve(env: Env, token: Address, to: Address, amount: i128) {
        token.transfer(&env.current_contract_address(), &to, &amount);
    }

    // Violation: treasury sweep has admin auth but no nonce/replay guard.
    pub fn sweep_treasury(env: Env, admin: Address, token: Address, to: Address, amount: i128) {
        admin.require_auth();
        token.transfer(&env.current_contract_address(), &to, &amount);
    }

    // Clean: admin-signed withdrawal is bound to a nonce that is checked and recorded.
    pub fn withdraw_treasury_with_nonce(
        env: Env,
        admin: Address,
        token: Address,
        to: Address,
        amount: i128,
        nonce: u64,
    ) {
        admin.require_auth_for_args((Symbol::short("reserve"), nonce).into_val(&env));
        assert!(!is_nonce_used(&env, nonce));
        record_nonce(&env, nonce);
        token.transfer(&env.current_contract_address(), &to, &amount);
    }
}
