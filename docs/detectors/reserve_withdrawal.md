# reserve_withdrawal

| Code | Category | Severity |
| --- | --- | --- |
| `S023` | authorization | High |

## What it catches

This detector identifies missing strict authorization guards on functions that withdraw, transfer, or modify critical reserve and treasury funds. 

Smart contracts often hold funds in reserve (e.g., protocol fees, locked collateral, or liquidity reserves). Functions that allow withdrawing from these reserves must ensure that the caller is strictly authorized (typically an administrator or a governance contract). If a withdrawal function lacks `require_auth()` or a similar explicit check, malicious actors could drain the protocol's reserves.

## Vulnerable example

```rust
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct Treasury;

#[contractimpl]
impl Treasury {
    pub fn withdraw_reserve(env: Env, admin: Address, to: Address, amount: i128) {
        // VULNERABLE: Missing `admin.require_auth()`
        // Anyone can call this and withdraw to their own address
        
        let token = get_token(&env);
        token.transfer(&env.current_contract_address(), &to, &amount);
    }
}
```

## The fix

Require explicit authorization from the trusted administrator or governance role before transferring any reserve funds.

```rust
#[contractimpl]
impl Treasury {
    pub fn withdraw_reserve(env: Env, admin: Address, to: Address, amount: i128) {
        // SAFE: Administrator must authorize the withdrawal
        admin.require_auth();
        
        let token = get_token(&env);
        token.transfer(&env.current_contract_address(), &to, &amount);
    }
}
```

## How Sanctifier detects it

Sanctifier looks for public entrypoints that perform token transfers or balance deductions on the contract's own address (`env.current_contract_address()`) but do not invoke `require_auth()` on an administrator or caller argument. It specifically flags entrypoints matching common reserve/treasury withdrawal patterns (e.g., function names containing `withdraw`, `claim`, `rescue`, `reserve`, `treasury`).

## References

* [Soroban Authorization Documentation](https://soroban.stellar.org/docs/learn/authorization)
