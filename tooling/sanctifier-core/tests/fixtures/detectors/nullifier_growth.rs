#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

#[contract]
pub struct ShieldedPool;

#[contractimpl]
impl ShieldedPool {
    // VULNERABLE: spends a note by marking its nullifier spent in persistent
    // storage, but the entry is never pruned, TTL-extended, or size-capped.
    pub fn spend_note(env: Env, nullifier: BytesN<32>, proof: BytesN<192>) {
        verify_note_proof(&env, &proof);
        env.storage().persistent().set(&nullifier, &true);
    }

    // VULNERABLE: same footgun under a differently-named ZK entrypoint.
    pub fn verify_and_claim(env: Env, nullifier: BytesN<32>, recipient: Address) {
        env.storage().persistent().set(&nullifier, &true);
        pay_out(&env, &recipient);
    }

    // OK: the nullifier's TTL is extended after it is written, so the entry
    // is not left to accumulate indefinitely without an expiry.
    pub fn spend_note_with_ttl(env: Env, nullifier: BytesN<32>) {
        env.storage().persistent().set(&nullifier, &true);
        env.storage().persistent().extend_ttl(&nullifier, 100, 1_000);
    }

    // OK: bounded because a stale nullifier is pruned on every spend.
    pub fn spend_note_pruned(env: Env, nullifier: BytesN<32>, stale: BytesN<32>) {
        env.storage().persistent().set(&nullifier, &true);
        env.storage().persistent().remove(&stale);
    }

    // OK: an explicit length cap on the tracked nullifier set guards growth.
    pub fn spend_note_capped(env: Env, nullifier: BytesN<32>, tracked: Vec<BytesN<32>>) {
        if tracked.len() < 1_000_000 {
            env.storage().persistent().set(&nullifier, &true);
        }
    }

    // OK: persists state in a spend-shaped entrypoint, but the key is not a
    // nullifier/commitment (it's an unrelated balance key).
    pub fn withdraw(env: Env, who: Address, amount: i128) {
        env.storage().persistent().set(&who, &amount);
    }

    // OK: writes a nullifier key, but from a function whose name gives no
    // spend/verify/nullify hint, so it is out of scope for this rule.
    pub fn record(env: Env, nullifier: BytesN<32>) {
        env.storage().persistent().set(&nullifier, &true);
    }
}

fn verify_note_proof(_env: &Env, _proof: &BytesN<192>) {}
fn pay_out(_env: &Env, _recipient: &Address) {}
