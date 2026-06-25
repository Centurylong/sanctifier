# sanctifier-guards

Runtime guard macros for Soroban smart contracts. Companion to `sanctifier-core`, which handles the static analysis side of the same problem.

`guard_invariant!(env, cond, Error::X)` checks a runtime invariant and, on failure, publishes a structured event before trapping the transaction. The event is committed to the on-chain ledger *before* the trap rolls back state, so off-chain indexers always see the violation even though the contract's storage writes are reverted.

This bridges static analysis and runtime defence. Static rules catch what you can prove at compile time; the macro catches what you cannot, and leaves a queryable on-chain audit trail when it does.

## Usage

```rust,ignore
use soroban_sdk::{contract, contractimpl, contracterror, Env};
use sanctifier_guards::guard_invariant;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    SupplyMismatch = 1,
}

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn settle(env: Env, total: u128, sum_of_balances: u128) {
        guard_invariant!(&env, total == sum_of_balances, Error::SupplyMismatch);
        // ... rest of the function only runs when the invariant holds.
    }
}
```

When the condition is false the macro:

1. Captures the condition into a local binding so it is evaluated exactly once.
2. Publishes a single event with topic `inv_fail` and a data payload of `(symbol_short!("cond"), String::from_str(env, stringify!(cond)))`. The condition source is in the payload, not the topics, so indexers only need to subscribe to one topic across every contract.
3. Calls `panic_with_error!(env, err)` to trap. The user-supplied `err` value carries a typed contract error code rather than a stringly typed panic message.

## Companion macros

### `guard_invariant_result!`

Returns `Err($err)` instead of trapping. Useful where the caller's signature is already `Result<_, Error>` (such as the wrapper's `execute_guarded`) and the typed error should bubble through `?` rather than abort the function mid-execution. Same event publishing semantics as the trapping form.

```rust,ignore
use sanctifier_guards::guard_invariant_result;

fn checked(env: Env) -> Result<(), Error> {
    guard_invariant_result!(&env, supply_conserved, Error::SupplyMismatch);
    Ok(())
}
```

### `guard_invariant_pass!`

Publishes a single `inv_pass` event without trapping. Useful when a wrapper wants to record successful post-condition checks alongside violations.

## Design notes

- **One topic per event.** Topics are a Soroban scarce resource; the condition source rides in the data payload. This keeps the indexer story simple: subscribe to `inv_fail`, decode the data payload, and you get every invariant failure across every contract using the macro.
- **Exactly-once condition evaluation.** The macro captures `$cond` into a local before branching, so callers can pass expressions with side effects without accidentally double-charging gas or miscounting storage state.
- **Event before trap.** The audit trail is the point. The host commits the event into the transaction's event set even though the subsequent `panic_with_error!` rolls back contract state, so violations are queryable after the fact.
- **Topic length budget.** `symbol_short!` allows up to nine bytes. Both `inv_fail` and `inv_pass` fit at eight, and there is a compile-time test that catches future renames that would break the budget.
- **No proc-macro dependency.** Declarative `macro_rules!` only. Adding a proc-macro variant (for richer compile-time diagnostics or span pointing) is possible without breaking call sites if reviewers want it later.

## Testing the trap path

`guard_invariant!` calls `panic_with_error!` which, in soroban-sdk 20.5's testutils, raises a non-unwinding panic that aborts the test runner before `#[should_panic]` or `std::panic::catch_unwind` can rescue it. The `guard_invariant_result!` companion has identical event-publishing semantics with a typed error return, so every assertion about event payload, condition source, and error code stays observable in unit tests by exercising the Result form. The trap path is then verified end to end in the `runtime-guard-wrapper` integration suite, where the wrapper's outer `Result<Val, Error>` signature bubbles the typed error to the caller.

## Linkage with the static side

`sanctify::invariant(...)` (from `tooling/sanctify-macros`) is the static counterpart that feeds `sanctifier-core`'s SMT backend at analysis time. `guard_invariant!` is the runtime counterpart. They share the same vocabulary on purpose so a human grepping logs for `inv_fail` finds both the static finding and the runtime event.

## License

MIT
