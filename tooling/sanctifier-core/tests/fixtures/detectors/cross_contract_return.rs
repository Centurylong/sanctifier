use soroban_sdk::{contractimpl, Address, Env, Symbol, Vec};

pub struct Contract;

pub struct TokenClient;

impl TokenClient {
    pub fn new(_env: &Env, _token: &Address) -> Self {
        Self
    }

    pub fn transfer(&self, _from: &Address, _to: &Address, _amount: &i128) -> bool {
        true
    }

    pub fn balance(&self, _id: &Address) -> i128 {
        0
    }
}

#[contractimpl]
impl Contract {
    pub fn unchecked(env: Env, token: Address, from: Address, to: Address) {
        let token_client = TokenClient::new(&env, &token);

        token_client.transfer(&from, &to, &10);
        env.invoke_contract::<i128>(&token, &Symbol::new(&env, "balance"), Vec::new(&env));
        let _ = token_client.balance(&to);
    }

    pub fn checked(env: Env, token: Address, to: Address) -> i128 {
        let token_client = TokenClient::new(&env, &token);

        let balance = token_client.balance(&to);
        // sanctifier:ignore[SANCT_CROSS_CONTRACT_RETURN]
        token_client.transfer(&to, &to, &1);
        balance
    }
}
