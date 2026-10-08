# stale_state_postcall — Cached storage state after a cross-contract call

| | |
| --- | --- |
| **Finding code** | [SANCT_STALE_STATE_POSTCALL](../error-codes.md) |
| **Category** | reentrancy |
| **Severity** | Warning |
| **Source rule** | [stale_state_postcall.rs](../../tooling/sanctifier-core/src/rules/stale_state_postcall.rs) |

## What it catches

A function reads a value from Soroban instance, persistent or temporary storage,
keeps that value in a local variable, invokes another contract, and subsequently
uses the stale variable to decide a branch, match arm, or loop. A cross-contract
call can re-enter or otherwise change the contract state. A decision based on
old state may no longer reflect the storage contents after the call.

## Vulnerable example

~~~rust
pub fn withdraw(env: Env, receiver: Address) {
    let balance: i128 = env.storage().persistent().get(&Key::Balance).unwrap_or(0);
    env.invoke_contract::<()>(&receiver, &symbol_short!("callback"), vec![&env]);
    if balance > 0 {
        // Decision uses pre-callback balance.
        release_funds(&env);
    }
}
~~~

## Fix

Read the affected storage key again after external control returns, then use
that fresh value in any decision. Where practical, redesign the state machine
to commit required state before the external call (checks-effects-interactions),
with appropriate authorization and reentrancy controls.

~~~rust
pub fn withdraw(env: Env, receiver: Address) {
    env.invoke_contract::<()>(&receiver, &symbol_short!("callback"), vec![&env]);
    let current = env.storage().persistent().get(&Key::Balance).unwrap_or(0);
    if current > 0 {
        release_funds(&env);
    }
}
~~~

## Detection and boundaries

The rule analyzes each function using the Rust syntax tree, records local
storage-get/has bindings, notices direct invoke_contract or try_invoke_contract
and calls through generated Client constructors, and checks condition
expressions after the invocation. It tracks ordinary local assignment,
shadowing, and conditionally executed branches. Re-reading the value into
the same local or a fresh local after the call avoids this finding.

This is a conservative, **intraprocedural pattern detector**, not a proof of
reentrant execution. It does not follow state through separately defined
helper functions, closure captures, pointers, or opaque storage wrappers. Calls
on clients without a visible generated Client constructor may be missed.
Branch joins conservatively retain a possible stale path. A finding requires
human review of the relevant storage key and actual callable contract.

## References

- [Soroban contract storage](https://developers.stellar.org/docs/build/smart-contracts/persisting-data)
- [CWE-841: Improper Enforcement of Behavioral Workflow](https://cwe.mitre.org/data/definitions/841.html)
- [CWE-1265: Unintended Reentrant Invocation](https://cwe.mitre.org/data/definitions/1265.html)
