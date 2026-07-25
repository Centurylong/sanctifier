#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// FIXTURE: init_hardcoded_admin detector
// Demonstrates hardcoded admin addresses/bytes in init functions vs requiring admin: Address.

#[contract]
pub struct InitAdminContract;

#[contractimpl]
impl InitAdminContract {
    // Violation 1: Hardcoded Stellar admin address string in initialize function.
    pub fn initialize(env: Env) {
        let admin = "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";
        env.storage().instance().set(&"admin", &admin);
    }

    // Violation 2: Hardcoded byte array literal in init function without formal admin argument.
    pub fn init(env: Env) {
        let admin_bytes = b"01234567890123456789012345678901";
        env.storage().instance().set(&"admin", &admin_bytes);
    }

    // Compliant: Takes admin Address as a formal parameter.
    pub fn reinitialize(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&"admin", &admin);
    }
}
