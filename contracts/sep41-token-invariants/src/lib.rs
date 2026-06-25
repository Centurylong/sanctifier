#![no_std]

pub mod kani_proofs;
pub mod pure;

use sanctify_macros::invariant;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env};

use pure::{approve_pure, burn_pure, mint_pure, transfer_from_pure, transfer_pure};

// ── Storage keys ──────────────────────────────────────────────────────────────

const BALANCE: soroban_sdk::Symbol = symbol_short!("BAL");
const ALLOWANCE: soroban_sdk::Symbol = symbol_short!("ALLOW");
const SUPPLY: soroban_sdk::Symbol = symbol_short!("SUPPLY");
const ADMIN: soroban_sdk::Symbol = symbol_short!("ADMIN");

const NAME: soroban_sdk::Symbol = symbol_short!("NAME");
const SYMBOL: soroban_sdk::Symbol = symbol_short!("SYM");
const DECIMALS: soroban_sdk::Symbol = symbol_short!("DEC");

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct Sep41Token;

/// SEP-41 compliant token with formal verification invariants.
///
/// The `#[invariant]` attributes declare properties that must hold across all
/// state transitions. `sanctifier verify` reports them; `cargo kani` proves
/// them symbolically.
///
/// **Invariants**:
/// - Supply is conserved across all transfers
/// - Transfers and transfer-from conserve supply and enforce allowance
/// - Approve consistently sets allowance
/// - Mint and burn correctly modify supply
///
/// ## Using this as a template
///
/// 1. Copy `pure.rs` to your project — it has no Soroban dependencies.
/// 2. Copy the `#[invariant(...)]` attributes onto your `#[contractimpl]` block.
/// 3. Run `cargo kani` to prove the invariants symbolically.
/// 4. Run `sanctifier verify` for static analysis reporting.
#[invariant(pure::supply_conserved_after_transfer(0, 0, 0))]
#[invariant(pure::supply_conserved_after_transfer_from(0, 0, 0, 0))]
#[invariant(pure::allowance_is_set_by_approve(0))]
#[contractimpl]
impl Sep41Token {
    // ── Initialization ─────────────────────────────────────────────────────

    /// Initialize the token with metadata and mint initial supply to admin.
    pub fn initialize(
        env: Env,
        admin: Address,
        name: soroban_sdk::String,
        symbol: soroban_sdk::String,
        decimals: u32,
        initial_supply: i128,
    ) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&NAME, &name);
        env.storage().instance().set(&SYMBOL, &symbol);
        env.storage().instance().set(&DECIMALS, &decimals);
        env.storage().instance().set(&SUPPLY, &initial_supply);
        env.storage().persistent().set(&admin, &initial_supply);
    }

    // ── Metadata ───────────────────────────────────────────────────────────

    pub fn name(env: Env) -> soroban_sdk::String {
        env.storage().instance().get(&NAME).unwrap()
    }

    pub fn symbol(env: Env) -> soroban_sdk::String {
        env.storage().instance().get(&SYMBOL).unwrap()
    }

    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().get(&DECIMALS).unwrap()
    }

    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&ADMIN).unwrap()
    }

    pub fn set_admin(env: Env, new_admin: Address) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &new_admin);
    }

    // ── SEP-41 Transfer ────────────────────────────────────────────────────

    /// Transfer `amount` tokens from `from` to `to`.
    ///
    /// Delegates to `transfer_pure` for verified arithmetic.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let bal_from: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        let bal_to: i128 = env.storage().persistent().get(&to).unwrap_or(0);

        let (new_from, new_to) = transfer_pure(bal_from, bal_to, amount).expect("transfer failed");

        env.storage().persistent().set(&from, &new_from);
        env.storage().persistent().set(&to, &new_to);
    }

    // ── SEP-41 Approve / Allowance ─────────────────────────────────────────

    /// Approve `spender` to transfer up to `amount` from `from`.
    ///
    /// Delegates to `approve_pure` for verified validation.
    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        _live_until_ledger: u32,
    ) {
        from.require_auth();

        let _new_allowance = approve_pure(amount).expect("approve failed");

        env.storage()
            .persistent()
            .set(&(from.clone(), spender), &amount);
    }

    /// Return the allowance `spender` has from `from`.
    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&(from, spender))
            .unwrap_or(0)
    }

    // ── SEP-41 Transfer-From ───────────────────────────────────────────────

    /// Transfer `amount` tokens on behalf of `from` using allowance.
    ///
    /// Delegates to `transfer_from_pure` for verified arithmetic.
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        let allow: i128 = env
            .storage()
            .persistent()
            .get(&(from.clone(), spender.clone()))
            .unwrap_or(0);
        let bal_from: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        let bal_to: i128 = env.storage().persistent().get(&to).unwrap_or(0);

        let (new_from, new_to, new_allow) =
            transfer_from_pure(bal_from, bal_to, allow, amount).expect("transfer_from failed");

        env.storage().persistent().set(&from, &new_from);
        env.storage().persistent().set(&to, &new_to);
        env.storage().persistent().set(&(from, spender), &new_allow);
    }

    // ── SEP-41 Burn / Burn-From ────────────────────────────────────────────

    /// Burn `amount` tokens from `from`. Admin-only.
    pub fn burn(env: Env, from: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        let bal: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        let supply: i128 = env.storage().instance().get(&SUPPLY).unwrap_or(0);

        let new_bal = burn_pure(bal, amount).expect("burn failed");
        let new_supply = burn_pure(supply, amount).expect("supply underflow");

        env.storage().persistent().set(&from, &new_bal);
        env.storage().instance().set(&SUPPLY, &new_supply);
    }

    /// Burn `amount` tokens from `from` using allowance. Caller uses their
    /// allowance to burn on behalf of `from`.
    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();

        let allow: i128 = env
            .storage()
            .persistent()
            .get(&(from.clone(), spender.clone()))
            .unwrap_or(0);
        let bal: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        let supply: i128 = env.storage().instance().get(&SUPPLY).unwrap_or(0);

        if allow < amount {
            panic!("insufficient allowance to burn");
        }
        let new_bal = burn_pure(bal, amount).expect("burn failed");
        let new_supply = burn_pure(supply, amount).expect("supply underflow");
        let new_allow = allow - amount;

        env.storage().persistent().set(&from, &new_bal);
        env.storage().instance().set(&SUPPLY, &new_supply);
        env.storage().persistent().set(&(from, spender), &new_allow);
    }

    // ── SEP-41 Mint ────────────────────────────────────────────────────────

    /// Mint `amount` tokens to `to`. Admin-only.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        let bal: i128 = env.storage().persistent().get(&to).unwrap_or(0);
        let supply: i128 = env.storage().instance().get(&SUPPLY).unwrap_or(0);

        let new_bal = mint_pure(bal, amount).expect("mint failed");
        let new_supply = mint_pure(supply, amount).expect("supply overflow");

        env.storage().persistent().set(&to, &new_bal);
        env.storage().instance().set(&SUPPLY, &new_supply);
    }

    // ── Queries ────────────────────────────────────────────────────────────

    /// Return the balance of `account`.
    pub fn balance(env: Env, account: Address) -> i128 {
        env.storage().persistent().get(&account).unwrap_or(0)
    }

    /// Return the total token supply.
    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().get(&SUPPLY).unwrap_or(0)
    }
}

// ── Pure invariant tests ──────────────────────────────────────────────────────
// Integration tests (requiring env.register()) are in tests/integration_tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supply_conserved_pure() {
        assert!(pure::supply_conserved_after_transfer(1_000, 0, 500));
        assert!(pure::supply_conserved_after_transfer(100, 900, 100));
        assert!(pure::supply_conserved_after_transfer(50, 50, 0)); // invalid → no-op
    }

    #[test]
    fn test_transfer_from_conserved_pure() {
        assert!(pure::supply_conserved_after_transfer_from(100, 50, 60, 25));
    }

    #[test]
    fn test_allowance_consistency_pure() {
        assert!(pure::allowance_is_set_by_approve(500));
    }
}
