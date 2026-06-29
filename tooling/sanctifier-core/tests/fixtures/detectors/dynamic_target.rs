use soroban_sdk::{contract, contractimpl, Address, Env, Bytes, Vec, symbol_short};

#[contract]
pub struct ProxyContract;

#[contractimpl]
impl ProxyContract {
    // ❌ Vulnerable: untrusted target called without check via Client
    pub fn proxy(env: Env, target: Address, data: Bytes) {
        let client = TokenClient::new(&env, &target);
        let _res = client.transfer(&env.current_contract_address(), &target, &100);
    }

    // ❌ Vulnerable: untrusted target called without check via direct invoke
    pub fn proxy_invoke(env: Env, target: Address, data: Bytes) {
        let _res: Bytes = env.invoke_contract(&target, &symbol_short!("exec"), vec![&env]);
    }

    // ❌ Vulnerable: untrusted target copied to alias and then instantiated without check
    pub fn proxy_alias(env: Env, target: Address, data: Bytes) {
        let dst = target.clone();
        let client = TokenClient::new(&env, &dst);
        let _res = client.transfer(&env.current_contract_address(), &dst, &100);
    }

    // ✅ Secure: checked via allowlist.contains() method call
    pub fn proxy_secure_allowlist(env: Env, target: Address, data: Bytes, allowlist: Vec<Address>) {
        if !allowlist.contains(&target) {
            panic!("Not allowed");
        }
        let client = TokenClient::new(&env, &target);
        let _res = client.transfer(&env.current_contract_address(), &target, &100);
    }

    // ✅ Secure: checked via binary comparison (target != allowed)
    pub fn proxy_secure_compare(env: Env, target: Address, allowed: Address, data: Bytes) {
        if target != allowed {
            panic!("Not allowed");
        }
        let client = TokenClient::new(&env, &target);
        let _res = client.transfer(&env.current_contract_address(), &target, &100);
    }

    // ✅ Secure: checked via storage get/has lookup
    pub fn proxy_secure_storage(env: Env, target: Address, data: Bytes) {
        let allowed: Address = env.storage().instance().get(&symbol_short!("allowed")).unwrap();
        if target == allowed {
            let client = TokenClient::new(&env, &target);
            let _res = client.transfer(&env.current_contract_address(), &target, &100);
        }
    }
}
