//! Fixture for the `unbounded_input_length` detector.

use soroban_sdk::{contractimpl, Bytes, BytesN, Env, Map, Vec};

pub struct Registry;

#[contractimpl]
impl Registry {
    /// FLAGGED: caller-sized `Bytes` is hashed and stored, never length-checked.
    pub fn submit_blob(env: Env, blob: Bytes) {
        let digest = env.crypto().sha256(&blob);
        env.storage().persistent().set(&digest, &blob);
    }

    /// FLAGGED: a `Vec` that is stored wholesale without a cap. Note it is
    /// never iterated, so `arg_dos` does not see it.
    pub fn set_members(env: Env, members: Vec<u32>) {
        env.storage().instance().set(&0u32, &members);
    }

    /// FLAGGED: `Map` argument forwarded on with no bound.
    pub fn merge_config(env: Env, config: Map<u32, u32>) {
        env.storage().instance().set(&1u32, &config);
    }

    /// NOT FLAGGED: the length is capped before use.
    pub fn submit_blob_capped(env: Env, blob: Bytes) {
        if blob.len() > 1024 {
            panic!("blob too large");
        }
        let digest = env.crypto().sha256(&blob);
        env.storage().persistent().set(&digest, &blob);
    }

    /// NOT FLAGGED: `BytesN<32>` is fixed-width by construction — the bound is
    /// carried in the type, so there is nothing for the caller to inflate.
    pub fn submit_hash(env: Env, hash: BytesN<32>) {
        env.storage().persistent().set(&hash, &true);
    }

    /// NOT FLAGGED: the argument is never used, so it cannot exhaust anything.
    pub fn ignores_input(_env: Env, _unused: Bytes) {}

    /// NOT FLAGGED: not a public entrypoint.
    fn internal_helper(env: Env, blob: Bytes) {
        env.storage().persistent().set(&0u32, &blob);
    }
}
