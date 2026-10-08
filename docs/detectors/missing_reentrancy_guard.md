# `missing_reentrancy_guard` — Unguarded value-moving cross-contract calls

| | |
| --- | --- |
| **Finding code** | [`SANCT_MISSING_REENTRANCY_GUARD`](../error-codes.md) |
| **Category** | reentrancy |
| **Severity** | Warning |
| **Source rule** | [`rules/missing_reentrancy_guard.rs`](../../tooling/sanctifier-core/src/rules/missing_reentrancy_guard.rs) |

## What it catches

A public, value-moving Soroban contract entrypoint invokes another contract without an active `SanctifiedGuard` or `ReentrancyGuard`. An external call introduces an interaction boundary: a callback may reach the invoking contract before its state transition has completed. The rule highlights entrypoints that warrant a guard and a checks-effects-interactions (CEI) review; it does **not** assert that an exploit is proven.

## Vulnerable example

```rust
impl Vault {
    pub fn withdraw(env: Env, token: Address, to: Address, amount: i128) {
        TokenClient::new(&env, &token).transfer(
            &env.current_contract_address(),
            &to,
            &amount,
        ); // value-moving external call with no active guard
    }
}
```

The rule emits `SANCT_MISSING_REENTRANCY_GUARD` at the external call in `withdraw`.

## The fix

Acquire a contract-appropriate guard before the interaction, maintain it through the relevant state transition, and release it only afterward. For example, using a **project-provided** `SanctifiedGuard` abstraction (not a type built into `soroban-sdk`):

```rust
impl Vault {
    pub fn withdraw(env: Env, token: Address, to: Address, amount: i128) {
        let _guard = SanctifiedGuard::enter(&env);
        // Verify authorization and balances, then apply required effects
        // before interacting where the contract's semantics permit.
        TokenClient::new(&env, &token).transfer(
            &env.current_contract_address(),
            &to,
            &amount,
        );
    }
}
```

Ensure the real guard implementation enforces a non-reentrant critical section on every relevant entrypoint. A guard name by itself does not make a contract safe. Favor CEI ordering and contract-specific authorization and state invariants as complementary controls.

## How Sanctifier detects it

The source-level, intraprocedural rule parses Rust with `syn` and examines public free functions and public impl methods (excluding `#[cfg(test)]` modules). It flags a function when both conditions hold:

1. The entrypoint name suggests a value transfer (such as `withdraw`, `redeem`, `mint`, or `swap`) **or** its body has a token-client `transfer`, `transfer_from`, `mint`, `burn`, `swap`, `send`, or `payout` method; and
2. A `env.invoke_contract(..)` or recognized `*Client` method call occurs while no recognized guard is active.

The visitor recognizes `SanctifiedGuard` and `ReentrancyGuard` constructor/entry patterns, explicit `enter`/`try_enter`/`acquire` versus `exit`/`release`/`unlock`, and `drop(guard)`/`std::mem::drop(guard)`/`core::mem::drop(guard)` when exactly one tracked RAII guard exists. Dropping only a reference (`drop(&guard)`) does not release it. Declarations of unused local functions/impls are not scanned as though they execute in the outer entrypoint. It reports the first unguarded external-call line per affected function.

### When it does not fire

- An active recognized guard surrounds the cross-contract call.
- A value-moving entrypoint modifies only local storage and makes no detected cross-contract call.
- A read-only/query-shaped entrypoint merely calls another contract without a detected value-moving operation.
- A test-only function is under `#[cfg(test)]`.

### Known limits

This is a **heuristic advisory**, not interprocedural control-flow or reentrancy proof. It does not trace helpers, aliases or macros across modules, establish that a guard enforces mutual exclusion, resolve dynamic call targets, or model every conditional execution path. Function-name heuristics and unknown `*Client` methods can yield false positives or false negatives. The current visitor's guard state is lexical and may not model conditional release, scope-bound destruction or exceptional exits precisely. Review findings against the actual contract and the separate CEI/order-of-effects analysis.

## References

- [Soroban smart contracts](https://developers.stellar.org/docs/build/smart-contracts/overview)
- [CWE-841: Improper Enforcement of Behavioral Workflow](https://cwe.mitre.org/data/definitions/841.html)
- [CWE-362: Concurrent Execution Using Shared Resource with Improper Synchronization](https://cwe.mitre.org/data/definitions/362.html)
- [Related rule: `cross_contract_call_in_loop`](cross_contract_call_in_loop.md)
