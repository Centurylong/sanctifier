#![no_std]

use soroban_sdk::{contract, contractimpl, contracterror, Address, Env};

// FIXTURE: Demonstrates hygiene rule violations

// Violation: SANCT_ERROR_CODES - Duplicate discriminants
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ErrorWithDuplicates {
    NotFound = 1,
    Invalid = 1,  // Duplicate!
    Unauthorized = 2,
}

// Violation: SANCT_ERROR_CODES - Inconsistent style
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ErrorInconsistent {
    NotFound = 1,
    Invalid,      // Implicit
    Unauthorized = 3,
}

#[contract]
pub struct HygieneViolations;

#[contractimpl]
impl HygieneViolations {
    // Violation: SANCT_HARDCODED_ADDR - Hardcoded admin address
    pub fn initialize(env: Env) {
        let admin = "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";
        env.storage().instance().set(&"admin", &admin);
    }

    // Violation: SANCT_HARDCODED_ADDR - Hardcoded secret key (critical!)
    pub fn verify_signature(env: Env) -> bool {
        let secret = "SA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";
        // Verify logic...
        true
    }

    // Violation: SANCT_EDGE_AMOUNT - Missing amount > 0 check
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        
        // Missing: if amount <= 0 { panic!(...) }
        // Missing: if from == to { panic!(...) }
        
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

        env.storage().persistent().set(&from, &(from_balance - amount));
        env.storage().persistent().set(&to, &(to_balance + amount));
    }

    // Violation: SANCT_EDGE_AMOUNT - Missing amount check in mint
    pub fn mint(env: Env, to: Address, amount: i128) {
        // Missing: if amount <= 0 { panic!(...) }
        
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&to)
            .unwrap_or(0);
        env.storage().persistent().set(&to, &(balance + amount));
    }

    // Violation: SANCT_EDGE_AMOUNT - Missing amount check in burn
    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        
        // Missing: if amount <= 0 { panic!(...) }
        
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&from)
            .unwrap_or(0);
        env.storage().persistent().set(&from, &(balance - amount));
    }
}
