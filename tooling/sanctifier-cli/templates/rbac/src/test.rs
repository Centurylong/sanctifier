#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn test_rbac() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, RBACContract);
    let client = RBACContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    client.grant_role(&user);
    client.execute_restricted(&user);
}
