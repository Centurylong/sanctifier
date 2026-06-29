use soroban_sdk::{Address, Env, Map, Vec};

struct Contract;

impl Contract {
    pub fn uncapped_vec_airdrop(env: Env, recipients: Vec<Address>, amount: i128) {
        for recipient in recipients.iter() {
            mint(&env, &recipient, amount);
        }
    }

    pub fn capped_vec_airdrop(env: Env, recipients: Vec<Address>, amount: i128) {
        if recipients.len() > 100 {
            panic!("too many recipients");
        }

        for recipient in recipients.iter() {
            mint(&env, &recipient, amount);
        }
    }

    pub fn capped_map_batch(env: Env, weights: Map<Address, i128>) {
        assert!(weights.len() <= 50, "too many weights");

        for entry in weights.iter() {
            apply_weight(&env, entry);
        }
    }

    pub fn internal_vec_is_bounded(env: Env, owner: Address) {
        let mut keys = Vec::new(&env);
        keys.push_back(owner);

        for key in keys.iter() {
            touch(&env, key);
        }
    }

    fn private_helper_is_not_an_entrypoint(recipients: Vec<Address>) {
        for recipient in recipients.iter() {
            inspect(recipient);
        }
    }
}

fn mint(_env: &Env, _recipient: &Address, _amount: i128) {}
fn apply_weight(_env: &Env, _entry: (Address, i128)) {}
fn touch(_env: &Env, _key: Address) {}
fn inspect(_recipient: Address) {}
