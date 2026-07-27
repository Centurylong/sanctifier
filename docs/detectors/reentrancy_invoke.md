# `reentrancy_invoke` — `env.invoke_contract` before state effects

| | |
| --- | --- |
| **Finding code** | [`SANCT_REENTRANCY_INVOKE`](../error-codes.md) |
| **Category** | reentrancy |
| **Severity** | Warning |
| **Source rule** | [`rules/reentrancy_invoke.rs`](../../tooling/sanctifier-core/src/rules/reentrancy_invoke.rs) |
| **Glossary** | [Reentrancy](../glossary.md#reentrancy) · [CEI pattern](../glossary.md#checks-effects-interactions) |

## What it catches

A public function that calls `env.invoke_contract` (an **interaction**) **before**
it performs storage writes (the **effects** phase), violating the
Checks-Effects-Interactions (CEI) pattern. When your contract calls out to
another contract before updating its own state, the callee can re-enter your
contract and observe **stale** state, which may allow it to bypass business logic
— e.g., drain tokens after a transfer already deducted the balance from the
callee but before the sender's balance is updated.

## Vulnerable example

```rust
#[contractimpl]
impl Vault {
    // Invoke before state write — reentrancy window.
    pub fn withdraw(env: Env, who: Address, amount: i128) {
        who.require_auth();
        let key = DataKey::Balance(who.clone());
        let bal: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if bal < amount {
            panic!("insufficient balance");
        }
        // INTERACTION first — the token contract re-enters `withdraw`.
        env.invoke_contract(&token_id, &symbol_short!("transfer"), vec![&env, who.clone(), amount.into()]);
        // EFFECT only after: balance is debited *after* the external call.
        env.storage().persistent().set(&key, &(bal - amount));
    }
}
```

## The fix

Move all storage writes **before** the external call. If the call fails, the
state change is reverted automatically by the Soroban host, so writing first is
safe:

```rust
#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, who: Address, amount: i128) {
        who.require_auth();
        let key = DataKey::Balance(who.clone());
        let bal: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if bal < amount {
            panic!("insufficient balance");
        }
        // EFFECT first: debit the balance.
        env.storage().persistent().set(&key, &(bal - amount));
        // INTERACTION after: safe, caller state is already updated.
        env.invoke_contract(&token_id, &symbol_short!("transfer"), vec![&env, who.clone(), amount.into()]);
    }
}
```

If the interaction **must** happen before the effect for architectural reasons,
consider using a reentrancy guard (e.g., [`reentrancy-guard`](../../contracts/reentrancy-guard/))
or a pull-over-push pattern.

## How Sanctifier detects it

The rule walks every public function in `#[contractimpl]` blocks with a
`syn::visit::Visit` pass. It records the line of the **first** `env.invoke_contract`
call and the line of the **first** storage mutation (`set`/`update`/`remove`/`try_update`).
If the invoke line precedes the effect line, the violation is emitted.

`#[cfg(test)]` modules are skipped, and functions with no storage effects are
considered out of scope (no CEI violation if there is nothing to re-enter over).

**Limitations:** Cross-function call chains are not analysed — if `fn_a` invokes
and `fn_b` writes, the rule does not connect them. False negatives are possible
for writes hidden behind abstraction, and false positives may occur for
genuinely idempotent interactions. Rename or suppress with
`// sanctifier:ignore[SANCT_REENTRANCY_INVOKE]`.

## References

- [SWC-107: Reentrancy](https://swcregistry.io/docs/SWC-107/)
- Soroban docs — [Contract Interactions](https://soroban.stellar.org/docs/how-to-guides/interacting-with-contracts)
- Related: [`state_write_in_view`](state_write_in_view.md), [`auth_gap`](auth_gap.md)
