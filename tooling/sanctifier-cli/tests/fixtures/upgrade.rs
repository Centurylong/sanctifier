#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Address};

#[contract]
pub struct UpgradeContract;

#[contractimpl]
impl UpgradeContract {
    pub fn initialize(env: Env, admin: Address) {
        env.storage().instance().set(&"admin", &admin);
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        // Direct upgrade without timelock or multisig
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    pub fn set_admin(env: Env, new_admin: Address) {
        env.storage().instance().set(&"admin", &new_admin);
    }
}
