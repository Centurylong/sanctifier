#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: hardcoded_addr detector
// Embeds a hardcoded Stellar public address and a hardcoded secret key.

#[contract]
pub struct HardcodedAddrContract;

#[contractimpl]
impl HardcodedAddrContract {
    // Violation: hardcoded admin address baked into the contract.
    pub fn initialize(env: Env) {
        let admin = "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";
        env.storage().instance().set(&"admin", &admin);
    }

    // Violation (critical): hardcoded secret seed in source.
    pub fn verify_signature(_env: Env) -> bool {
        let secret = "SA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";
        secret.len() > 0
    }
}
