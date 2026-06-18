#![no_std]

pub mod kani_proofs;
pub mod pure;

use sanctify_macros::invariant;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env};

use pure::{burn_pure, mint_pure, transfer_pure};

// ── Storage keys ──────────────────────────────────────────────────────────────

const BALANCE: soroban_sdk::Symbol = symbol_short!("BAL");
const SUPPLY: soroban_sdk::Symbol = symbol_short!("SUPPLY");
const ADMIN: soroban_sdk::Symbol = symbol_short!("ADMIN");

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct Token;

/// The `#[sanctify::invariant]` attribute declares that `supply_is_conserved`
/// must hold across all state transitions. In a normal build the attribute is
/// transparent. Under `cargo kani` it additionally emits a `#[kani::proof]`
/// harness; `sanctifier verify` reports the invariant in its output.
#[invariant(pure::supply_is_conserved_after_transfer(0, 0, 0))]
#[contractimpl]
impl Token {
    /// One-time initialisation. Sets the admin and mints the initial supply.
    pub fn initialize(env: Env, admin: Address, initial_supply: i128) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&SUPPLY, &initial_supply);
        env.storage()
            .persistent()
            .set(&admin, &initial_supply);
    }

    /// Transfer `amount` tokens from `from` to `to`.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let bal_from: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        let bal_to: i128 = env.storage().persistent().get(&to).unwrap_or(0);

        let (new_from, new_to) =
            transfer_pure(bal_from, bal_to, amount).expect("transfer failed");

        env.storage().persistent().set(&from, &new_from);
        env.storage().persistent().set(&to, &new_to);
    }

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

    /// Return the balance of `account`.
    pub fn balance(env: Env, account: Address) -> i128 {
        env.storage().persistent().get(&account).unwrap_or(0)
    }

    /// Return the total token supply.
    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().get(&SUPPLY).unwrap_or(0)
    }

    /// Return the stored BALANCE symbol — used to verify key constants compile.
    pub fn balance_key(_env: Env) -> soroban_sdk::Symbol {
        BALANCE
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_initialize_sets_supply() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Token, ());
        let client = TokenClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &1_000_000i128);
        assert_eq!(client.total_supply(), 1_000_000i128);
    }

    #[test]
    fn test_transfer_moves_balance() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Token, ());
        let client = TokenClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        client.initialize(&admin, &1_000_000i128);
        client.mint(&alice, &500i128);
        let bob = Address::generate(&env);
        client.transfer(&alice, &bob, &200i128);
        assert_eq!(client.balance(&alice), 300i128);
        assert_eq!(client.balance(&bob), 200i128);
    }

    #[test]
    fn test_supply_conserved_pure() {
        assert!(pure::supply_is_conserved_after_transfer(1_000, 0, 500));
        assert!(pure::supply_is_conserved_after_transfer(100, 900, 100));
        // Invalid transfer — function returns true (no-op)
        assert!(pure::supply_is_conserved_after_transfer(50, 50, 0));
    }
}
