#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol, Vec};

// FIXTURE: cei_violation detector (SANCT_CEI)
// A cross-contract call, including a token / Stellar Asset Contract (SAC)
// transfer, runs before a later storage write on the same path, so a callback
// can re-enter while the contract's own state is still stale.

#[contracttype]
pub enum DataKey {
    Balance(Address),
    Locked,
}

const PAID: Symbol = symbol_short!("PAID");

#[contract]
pub struct CeiVaultContract;

#[contractimpl]
impl CeiVaultContract {
    // Violation: SAC transfer through a bound token client before the debit.
    pub fn withdraw(env: Env, user: Address, token: Address, amount: i128) {
        user.require_auth();
        let key = DataKey::Balance(user.clone());
        let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let sac = token::Client::new(&env, &token);
        sac.transfer(&env.current_contract_address(), &user, &amount);
        env.storage().persistent().set(&key, &(balance - amount));
    }

    // Violation: inline token client transfer_from before the credit.
    pub fn deposit(env: Env, from: Address, token: Address, amount: i128) {
        from.require_auth();
        let key = DataKey::Balance(from.clone());
        let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        token::Client::new(&env, &token).transfer_from(
            &env.current_contract_address(),
            &from,
            &env.current_contract_address(),
            &amount,
        );
        env.storage().persistent().set(&key, &(balance + amount));
    }

    // Violation: host invoke_contract before the instance lock is released.
    pub fn call_hook(env: Env, hook: Address) {
        env.invoke_contract::<()>(&hook, &symbol_short!("on_call"), Vec::new(&env));
        env.storage().instance().set(&DataKey::Locked, &false);
    }

    // Safe (CEI order): the debit is committed before the SAC transfer.
    pub fn withdraw_safe(env: Env, user: Address, token: Address, amount: i128) {
        user.require_auth();
        let key = DataKey::Balance(user.clone());
        let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(balance - amount));
        let sac = token::Client::new(&env, &token);
        sac.transfer(&env.current_contract_address(), &user, &amount);
    }

    // Safe: the transfer and the write are on mutually exclusive branches.
    pub fn pay_or_record(env: Env, user: Address, token: Address, amount: i128, pay_now: bool) {
        user.require_auth();
        if pay_now {
            token::Client::new(&env, &token).transfer(&env.current_contract_address(), &user, &amount);
        } else {
            env.storage().persistent().set(&DataKey::Balance(user), &amount);
        }
    }

    // Safe: an event publish is not a cross-contract call and cannot re-enter.
    pub fn record_paid(env: Env, user: Address, amount: i128) {
        user.require_auth();
        env.events().publish((PAID,), (user.clone(), amount));
        env.storage().persistent().set(&DataKey::Balance(user), &amount);
    }
}
