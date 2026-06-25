#![no_std]

use protected_vault::ProtectedVaultClient;
use soroban_sdk::Env;

#[test]
fn test_deposit_and_balance() {
    let env = Env::default();
    let contract_id = env.register_contract(None, protected_vault::ProtectedVault);
    let client = ProtectedVaultClient::new(&env, &contract_id);

    client.deposit(&100_i128);
    assert_eq!(client.balance(), 100_i128);
}

#[test]
fn test_sequential_deposits_and_withdraw() {
    let env = Env::default();
    let contract_id = env.register_contract(None, protected_vault::ProtectedVault);
    let client = ProtectedVaultClient::new(&env, &contract_id);

    client.deposit(&200_i128);
    client.deposit(&50_i128);
    assert_eq!(client.balance(), 250_i128);

    client.withdraw(&100_i128);
    assert_eq!(client.balance(), 150_i128);
}

/// Simulates a reentrancy attack: the guard must detect the re-entry and abort.
#[test]
#[should_panic(expected = "reentrancy detected")]
fn test_reentrant_withdraw_is_rejected() {
    let env = Env::default();
    let contract_id = env.register_contract(None, protected_vault::ProtectedVault);
    let client = ProtectedVaultClient::new(&env, &contract_id);

    client.deposit(&500_i128);
    // reentrant_withdraw holds the lock then calls withdraw — should panic.
    client.reentrant_withdraw(&100_i128);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_withdraw_exceeding_balance_panics() {
    let env = Env::default();
    let contract_id = env.register_contract(None, protected_vault::ProtectedVault);
    let client = ProtectedVaultClient::new(&env, &contract_id);

    client.deposit(&10_i128);
    client.withdraw(&100_i128);
}

#[test]
fn test_balance_is_zero_initially() {
    let env = Env::default();
    let contract_id = env.register_contract(None, protected_vault::ProtectedVault);
    let client = ProtectedVaultClient::new(&env, &contract_id);

    assert_eq!(client.balance(), 0_i128);
}
