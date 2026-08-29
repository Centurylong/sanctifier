//! Fixture for the `auth_replay` detector.
//!
//! A custom account's `__check_auth` is the entire auth story for that
//! account — with no nonce or expiry, a captured signature is valid forever.
//! Each variant lives on its own account type since `__check_auth` is a
//! fixed, single entry point per `CustomAccountInterface` impl.

use soroban_sdk::{contractimpl, Env, Hash, Vec};

pub struct VulnerableAccount;

#[contractimpl]
impl VulnerableAccount {
    /// FLAGGED: verifies the signature against the payload and nothing else.
    /// The same signed payload can be resubmitted indefinitely.
    pub fn __check_auth(
        env: Env,
        sig_payload: Hash<32>,
        sigs: Vec<Signature>,
        _ctx: Vec<Context>,
    ) -> Result<(), Error> {
        verify(&env, &sig_payload, &sigs)?;
        Ok(())
    }
}

pub struct NonceCheckedAccount;

#[contractimpl]
impl NonceCheckedAccount {
    /// NOT FLAGGED: reads, checks, and increments a stored nonce before
    /// accepting the signature.
    pub fn __check_auth(
        env: Env,
        sig_payload: Hash<32>,
        sigs: Vec<Signature>,
        _ctx: Vec<Context>,
    ) -> Result<(), Error> {
        verify(&env, &sig_payload, &sigs)?;
        let stored_nonce: u64 = env.storage().persistent().get(&DataKey::Nonce).unwrap_or(0);
        env.storage().persistent().set(&DataKey::Nonce, &(stored_nonce + 1));
        Ok(())
    }
}

pub struct ExpiryCheckedAccount;

#[contractimpl]
impl ExpiryCheckedAccount {
    /// NOT FLAGGED: binds the signed payload to an expiry ledger sequence.
    pub fn __check_auth(
        env: Env,
        sig_payload: Hash<32>,
        sigs: Vec<Signature>,
        _ctx: Vec<Context>,
    ) -> Result<(), Error> {
        verify(&env, &sig_payload, &sigs)?;
        let expiration_ledger: u32 = 0;
        if env.ledger().sequence() > expiration_ledger {
            panic!("signature expired");
        }
        Ok(())
    }
}
