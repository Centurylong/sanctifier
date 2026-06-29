#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
pub enum DataKey {
    Allowance(Address, Address),
    Balance(Address),
    Nonce(Address),
    State(Address),
    Admin,
}

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        // Safe to skip auth requirement here for the test if we only test transfer/mint
        // but let's intentionally leave mint and transfer vulnerable.
        admin.require_auth();
        e.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn mint(e: Env, to: Address, amount: i128) {
        // BUG: Missing require_auth!
        // let admin: Address = e.storage().instance().get(&DataKey::Admin).unwrap();
        // admin.require_auth();

        let mut balance = Self::read_balance(&e, to.clone());
        // BUG: Overflow vulnerability! Standard + instead of a method that might be safer (though i128 is big, we want the analyzer to flag standard operators if it does, or at least we test if missing auth is caught)
        balance = balance + amount;
        
        e.storage().persistent().set(&DataKey::Balance(to), &balance);
    }

    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        // BUG: Missing require_auth!
        // from.require_auth();

        let mut from_balance = Self::read_balance(&e, from.clone());
        let mut to_balance = Self::read_balance(&e, to.clone());

        // We use a panic here just to trigger panic detection as an UnsafePattern
        if amount < 0 {
            panic!("Amount cannot be negative");
        }

        // Potential overflow bugs too
        from_balance = from_balance - amount;
        to_balance = to_balance + amount;

        e.storage().persistent().set(&DataKey::Balance(from), &from_balance);
        e.storage().persistent().set(&DataKey::Balance(to), &to_balance);
    }
    
    // Unsafe math standard unwrap
    pub fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        let current_balance: i128 = e.storage().persistent().get(&DataKey::Balance(from.clone())).unwrap();
        let new_balance = current_balance - amount;
        e.storage().persistent().set(&DataKey::Balance(from), &new_balance);
    }

    fn read_balance(e: &Env, owner: Address) -> i128 {
        e.storage().persistent().get(&DataKey::Balance(owner)).unwrap_or(0)
    }
}
