# wrong_auth_args

| Code | Category | Severity |
| --- | --- | --- |
| `S023` | authentication | Medium |

## What it catches

This detector flags instances where an internal, non-public helper function uses `require_auth()` instead of `require_auth_for_args()`. 

In Soroban, `require_auth()` authorizes the *entire* contract invocation using the arguments passed to the top-level contract entrypoint. If an internal helper relies on `require_auth()`, it implicitly binds authorization to the arguments of whichever public function called it, rather than the specific arguments being processed by the helper. This can lead to authorization gaps if the helper's arguments differ from the parent's arguments, or if the helper is called multiple times with different arguments in a loop. To safely authenticate specific arguments within an internal function, `require_auth_for_args()` must be used.

## Vulnerable example

```rust
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn batch_mint(env: Env, admin: Address, to: Address, amounts: Vec<i128>) {
        admin.require_auth();
        for amount in amounts.into_iter() {
            // VULNERABLE: internal_mint uses require_auth() which only authenticates
            // the `amounts` Vec from the parent function, not the specific `amount`
            // being minted in this iteration.
            Self::internal_mint(&env, &admin, &to, amount);
        }
    }
}

impl Token {
    fn internal_mint(env: &Env, admin: &Address, to: &Address, amount: i128) {
        // This authorizes the top-level invocation arguments, not `amount`
        admin.require_auth(); 
        // ...
    }
}
```

## The fix

Use `require_auth_for_args()` in the internal helper function to explicitly bind the authorization signature to the specific arguments being processed.

```rust
impl Token {
    fn internal_mint(env: &Env, admin: &Address, to: &Address, amount: i128) {
        // SAFE: explicitly authorizes this specific amount
        admin.require_auth_for_args((to.clone(), amount));
        // ...
    }
}
```

## How Sanctifier detects it

Sanctifier traverses the AST of the contract and identifies all function definitions that are not marked as `pub` within a `#[contractimpl]` block. If it finds a method call to `require_auth()` (with 0 arguments) within the body of one of these non-public functions, it raises a warning. Public entrypoints are ignored because `require_auth()` correctly binds to their exact arguments.

## References

* [Soroban Authorization Documentation](https://soroban.stellar.org/docs/learn/authorization)
