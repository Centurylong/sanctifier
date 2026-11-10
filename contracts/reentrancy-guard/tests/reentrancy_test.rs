use reentrancy_guard::ReentrancyGuard;
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct ProtectedContract;

#[contractimpl]
impl ProtectedContract {
    pub fn do_something(env: Env) {
        let guard = ReentrancyGuard::new(&env);
        guard.enter();
        // Section protected from reentrancy
        guard.exit();
    }

    pub fn malicious_reentry(env: Env) {
        let guard = ReentrancyGuard::new(&env);
        guard.enter();
        // Maliciously call back into do_something via contract invocation
        let client = ProtectedContractClient::new(&env, &env.current_contract_address());
        client.do_something();
        guard.exit();
    }
}

#[test]
fn test_reentrancy_protection() {
    use reentrancy_guard::{enter_pure, GuardStatus};
    let status = enter_pure(GuardStatus::Unlocked).unwrap();
    assert_eq!(status, GuardStatus::Locked);
    assert_eq!(enter_pure(status), Err("reentrancy detected"));
}

#[test]
fn test_normal_usage() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ProtectedContract);
    let client = ProtectedContractClient::new(&env, &contract_id);

    client.do_something();
    client.do_something(); // Sequential calls should work
}
