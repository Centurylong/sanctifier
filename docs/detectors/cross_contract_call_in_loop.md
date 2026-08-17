# `cross_contract_call_in_loop` — Cross-contract call issued inside a loop

| | |
| --- | --- |
| **Finding code** | [`SANCT_CROSS_CONTRACT_CALL_IN_LOOP`](../error-codes.md) |
| **Category** | resource_limits |
| **Severity** | Warning |
| **Source rule** | [`rules/cross_contract_call_in_loop.rs`](../../tooling/sanctifier-core/src/rules/cross_contract_call_in_loop.rs) |

## What it catches

A cross-contract invocation — `env.invoke_contract(..)`, or a call through a
generated `*Client` (`TokenClient::new(&env, &addr).transfer(..)`) — issued
from inside a `for`, `while`, or `loop` body. Each cross-contract call is a
separate host-to-host invocation with its own CPU instruction and memory
budget accounting. Issuing one per loop iteration over a caller-influenced
collection lets a single transaction's resource cost scale with that
collection's size, risking a resource-limit abort — or, short of an abort, a
denial-of-service for the rest of the transaction once the shared budget is
exhausted.

## Vulnerable example

```rust
#[contractimpl]
impl Airdrop {
    // Cost of this call scales with `targets.len()`; a large enough list
    // blows the transaction's cross-contract-call budget.
    pub fn broadcast(env: Env, targets: Vec<Address>) {
        for target in targets.iter() {
            env.invoke_contract::<()>(&target, &Symbol::new(&env, "ping"), Vec::new(&env));
        }
    }

    pub fn drain(env: Env, token: Address, mut recipients: Vec<Address>) {
        while let Some(to) = recipients.pop_front() {
            TokenClient::new(&env, &token).transfer(&env.current_contract_address(), &to, &1);
        }
    }
}
```

## The fix

Cap the number of iterations that can reach the call, or move the
cross-contract hop out of the loop entirely (e.g. batch into a single call the
downstream contract can fan out itself):

```rust
#[contractimpl]
impl Airdrop {
    pub fn broadcast(env: Env, targets: Vec<Address>) {
        const MAX_TARGETS: u32 = 25;
        if targets.len() > MAX_TARGETS {
            panic!("too many targets in one call");
        }
        for target in targets.iter() {
            env.invoke_contract::<()>(&target, &Symbol::new(&env, "ping"), Vec::new(&env));
        }
    }
}
```

## How Sanctifier detects it

The rule walks each `#[contractimpl]` function tracking loop nesting depth. A
method call named `invoke_contract`, or a method call chained off a
`<Ident>Client::new(..)` constructor, seen while that depth is greater than
zero is reported against the enclosing loop.

**Limitations:** it is a syntactic pattern match — it does not evaluate
whether the loop's bound is already a small, provably-fixed constant, and a
client constructed outside the loop and only invoked inside it may still be
missed if the construction and the call are not directly chained.

## References

- [SEP-0001: Base standards - resource limits](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering)
- Related: [`unbounded_event_emission`](unbounded_event_emission.md), [`unbounded_storage`](unbounded_storage.md)
