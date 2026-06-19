#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

// GALLERY: unchecked upgrade authorization
// Mapped finding: S010 (upgrade_risk) — currently surfaced by S001 (auth_gap),
// since the privileged mutation runs with no authentication.

const ADMIN: Symbol = symbol_short!("ADMIN");

#[contract]
pub struct UpgradeAuthVulnerable;

#[contractimpl]
impl UpgradeAuthVulnerable {
    // VULN: anyone can seize the contract by reassigning the upgrade admin —
    // the privileged write has no require_auth() gate.
    pub fn set_upgrade_admin(env: Env, new_admin: Address) {
        env.storage().instance().set(&ADMIN, &new_admin);
    }
}
