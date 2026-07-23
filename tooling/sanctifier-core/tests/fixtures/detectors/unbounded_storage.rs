#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Map, Symbol, Vec};

#[contract]
pub struct Registry;

#[contractimpl]
impl Registry {
    // VULNERABLE: append-only persistent Vec, never pruned or capped.
    pub fn register(env: Env, who: Address) {
        let key = Symbol::new(&env, "members");
        let mut members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        members.push_back(who);
        env.storage().persistent().set(&key, &members);
    }

    // VULNERABLE: append-only persistent Map keyed by caller, never pruned.
    pub fn record_score(env: Env, who: Address, score: i128) {
        let key = Symbol::new(&env, "scores");
        let mut scores: Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Map::new(&env));
        scores.set(who, score);
        env.storage().persistent().set(&key, &scores);
    }

    // OK: growth is guarded by an explicit length cap before the push.
    pub fn register_capped(env: Env, who: Address) {
        let key = Symbol::new(&env, "capped_members");
        let mut members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        if members.len() >= 100 {
            panic!("registry full");
        }
        members.push_back(who);
        env.storage().persistent().set(&key, &members);
    }

    // OK: bounded because a stale entry is pruned on every write.
    pub fn rotate(env: Env, who: Address) {
        let key = Symbol::new(&env, "rotating");
        let mut queue: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        queue.push_back(who);
        if queue.len() > 10 {
            queue.pop_front();
        }
        env.storage().persistent().set(&key, &queue);
    }

    // OK: purely local scratch collection, never persisted durably.
    pub fn tally(env: Env, first: Address, second: Address) -> u32 {
        let mut scratch: Vec<Address> = Vec::new(&env);
        scratch.push_back(first);
        scratch.push_back(second);
        scratch.len()
    }
}
