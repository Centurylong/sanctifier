#![no_std]

use soroban_sdk::{contract, contractimpl, contracterror, panic_with_error, Address, Env, Symbol};

// FIXTURE: Clean code without hygiene violations

// Clean: All explicit, unique discriminants
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ErrorClean {
    NotFound = 1,
    Invalid = 2,
    Unauthorized = 3,
}

#[contract]
pub struct HygieneClean;

#[contractimpl]
impl HygieneClean {
    // Clean: No hardcoded addresses, uses parameter
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&Symbol::new(&env, "admin"), &admin);
    }

    // Clean: Proper validation for transfer
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        
        // Proper amount validation
        if amount <= 0 {
            panic_with_error!(&env, ErrorClean::Invalid);
        }
        
        // Proper self-transfer check
        if from == to {
            panic_with_error!(&env, ErrorClean::Invalid);
        }
        
        let from_balance: i128 = env
            .storage()
            .persistent()
            .get(&from)
            .unwrap_or(0);
        let to_balance: i128 = env
            .storage()
            .persistent()
            .get(&to)
            .unwrap_or(0);

        if from_balance < amount {
            panic_with_error!(&env, ErrorClean::Invalid);
        }

        env.storage().persistent().set(&from, &(from_balance - amount));
        env.storage().persistent().set(&to, &(to_balance + amount));
    }

    // Clean: Proper amount validation in mint
    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) {
        admin.require_auth();
        
        // Proper validation
        if amount <= 0 {
            panic_with_error!(&env, ErrorClean::Invalid);
        }
        
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&to)
            .unwrap_or(0);
        env.storage().persistent().set(&to, &(balance + amount));
    }

    // Clean: Proper amount validation in burn
    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        
        // Proper validation
        if amount <= 0 {
            panic_with_error!(&env, ErrorClean::Invalid);
        }
        
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&from)
            .unwrap_or(0);

        if balance < amount {
            panic_with_error!(&env, ErrorClean::Invalid);
        }

        env.storage().persistent().set(&from, &(balance - amount));
    }
}
