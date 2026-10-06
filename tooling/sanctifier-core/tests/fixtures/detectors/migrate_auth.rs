//! Fixture for the `migrate_auth` detector.

use soroban_sdk::{Address, Env};

pub struct UnauthenticatedMigration;

impl UnauthenticatedMigration {
    // FLAGGED: public migration mutates state without authenticating an admin.
    pub fn migrate(env: Env) {
        env.storage().instance().set(&1, &2);
    }
}

pub struct AuthenticatedMigration;

impl AuthenticatedMigration {
    // NOT FLAGGED: the Address-typed administrator is explicitly authenticated.
    pub fn migrate(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&1, &2);
    }
}
