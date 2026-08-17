# `unbounded_event_emission` — Event published from inside a loop

| | |
| --- | --- |
| **Finding code** | [`SANCT_UNBOUNDED_EVENT_EMISSION`](../error-codes.md) |
| **Category** | resource_limits |
| **Severity** | Warning |
| **Source rule** | [`rules/unbounded_event_emission.rs`](../../tooling/sanctifier-core/src/rules/unbounded_event_emission.rs) |

## What it catches

An `env.events().publish(..)` call issued from inside a `for`, `while`, or
`loop` body. The number of events a transaction publishes (and the XDR/budget
cost of building and storing them) then scales with the loop's iteration
count instead of a fixed, reviewed bound. Over a large enough
caller-influenced collection this risks a resource-limit abort, and even
short of one it bloats every downstream consumer (indexers, RPC subscribers)
that has to process one event per loop iteration instead of a single summary.

## Vulnerable example

```rust
#[contractimpl]
impl Payroll {
    // One event per recipient — cost and downstream noise scale with
    // `recipients.len()`.
    pub fn notify_all(env: Env, recipients: Vec<Address>) {
        for to in recipients.iter() {
            env.events().publish((symbol_short!("paid"), to.clone()), amount);
        }
    }
}
```

## The fix

Publish a single summary event after the loop, or cap the number of
iterations that can reach `publish`:

```rust
#[contractimpl]
impl Payroll {
    pub fn notify_all(env: Env, recipients: Vec<Address>) {
        for to in recipients.iter() {
            // ... pay `to` ...
        }
        env.events().publish((symbol_short!("paid_all"),), recipients.len());
    }
}
```

## How Sanctifier detects it

The rule walks each `#[contractimpl]` function tracking loop nesting depth. A
`.publish(..)` method call whose receiver chain resolves through `.events()`,
seen while that depth is greater than zero, is reported against the enclosing
loop.

**Limitations:** it is a syntactic pattern match — it does not evaluate
whether the loop's bound is already a small, provably-fixed constant, so a
loop that is genuinely capped (e.g. `for i in 0..3`) is still flagged.

## References

- [Stellar docs — Events](https://developers.stellar.org/docs/learn/fundamentals/contract-development/events)
- Related: [`cross_contract_call_in_loop`](cross_contract_call_in_loop.md), [`unbounded_storage`](unbounded_storage.md)
