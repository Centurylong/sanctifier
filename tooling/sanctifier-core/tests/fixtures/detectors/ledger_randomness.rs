use soroban_sdk::{BytesN, Env};

struct Contract;

impl Contract {
    pub fn pick_winner(env: Env, participant_count: u32) -> u32 {
        env.ledger().sequence() % participant_count
    }

    pub fn random_seed(env: Env) -> BytesN<32> {
        hash(env.ledger().timestamp())
    }

    pub fn expires_at(env: Env, ttl: u64) -> u64 {
        env.ledger().timestamp() + ttl
    }
}
