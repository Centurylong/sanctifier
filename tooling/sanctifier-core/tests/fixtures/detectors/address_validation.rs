use soroban_sdk::{contractimpl, Address, Env};

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn set_admin(env: Env, admin: Address) {
        env.storage().instance().set(&"admin", &admin);
    }

    pub fn configure_asset(env: Env, asset: Address, recipient: Address) {
        env.storage().instance().set(&"asset", &asset);
        env.storage().instance().set(&"recipient", &recipient);
    }

    pub fn set_owner(env: Env, owner: Address) {
        validate_address(&owner);
        env.storage().instance().set(&"owner", &owner);
    }

    pub fn set_treasury(env: Env, treasury: Address) {
        if treasury.is_zero() {
            panic!("zero treasury");
        }
        env.storage().instance().set(&"treasury", &treasury);
    }

    // sanctifier:ignore[SANCT_ADDRESS_VALIDATION]
    pub fn set_token(env: Env, token: Address) {
        env.storage().instance().set(&"token", &token);
    }
}
