# `event_data_cast` — Lossy integer cast in event emission

| | |
| --- | --- |
| **Finding code** | [`SANCT_EVENT_DATA_CAST`](../error-codes.md) |
| **Category** | events |
| **Severity** | Warning |
| **Source rule** | [`rules/event_data_cast.rs`](../../tooling/sanctifier-core/src/rules/event_data_cast.rs) |
| **Glossary** | [Events](../glossary.md#events) · [Type narrowing](../glossary.md#type-narrowing) |

## What it catches

An integer `as`-cast inside `env.events().publish(…)` where the **target type is
narrower** (fewer bits) or has **different signedness** from the source. This
silently truncates the value that indexers and off-chain consumers receive,
leading to incorrect balances, amounts, or state in downstream analytics.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    pub fn deposit(env: Env, amount: i128) {
        env.events()
            .publish((TOPIC,), (amount as u32,));
        // i128 → u32 loses 96 bits AND signedness
    }
}
```

## The fix

Emit the full-width value and let off-chain consumers decide how to interpret it,
or cast through a checked conversion that panics on truncation:

```rust
#[contractimpl]
impl Token {
    pub fn deposit(env: Env, amount: i128) {
        // Full-width: no information loss.
        env.events().publish((TOPIC,), (amount,));
    }
}
```

## How Sanctifier detects it

The rule walks the AST of every function and, when it encounters an
`env.events().publish(…)` call, recursively inspects each event-data argument
for `as`-casts. A cast is flagged when:

- `target.bits() < source.bits()` (narrowing), **or**
- `target.signed() != source.signed()` (signedness change).

Widening casts (`u32 as u64`) and casts outside event context are ignored.

**Limitations:** the detector tracks types through `let` bindings but not across
function boundaries; an intermediate variable whose type was inferred from a
narrowing cast in a helper function will not be flagged.

## References

- Soroban — [Events](https://soroban.stellar.org/docs/getting-started/events)
- [CWE-681: Incorrect Conversion between Numeric Types](https://cwe.mitre.org/data/definitions/681.html)
- Related: [`shift_overflow`](shift_overflow.md)
