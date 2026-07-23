# `ledger_seconds` — Ledger sequence number confused with seconds

| | |
| --- | --- |
| **Finding code** | [`S021`](../error-codes.md) |
| **Category** | time_logic |
| **Severity** | Medium |
| **Source rule** | [`rules/ledger_seconds.rs`](../../tooling/sanctifier-core/src/rules/ledger_seconds.rs) |
| **Glossary** | [Ledger sequence vs. timestamp](../glossary.md) |

## What it catches

A ledger **sequence number** (`env.ledger().sequence()`) combined with an integer
literal that clearly denotes a duration in **seconds**. A Soroban ledger sequence
is a monotonic block counter that advances roughly once every ~5 seconds, so
`sequence() + 86_400` does **not** mean "one day from now" — it means 86,400
*ledgers*, i.e. several days of real time. Time windows expressed in seconds
belong with `env.ledger().timestamp()`.

## Vulnerable example

```rust
#[contractimpl]
impl Escrow {
    // Intends "1 day", but adds 86,400 ledgers (~5 days) to a block counter.
    pub fn deadline(env: Env) -> u32 {
        env.ledger().sequence() + 86400
    }
}
```

## The fix

Measure real-time windows against the wall-clock `timestamp()` (seconds), or
convert the duration to a ledger count before adding it to `sequence()`:

```rust
#[contractimpl]
impl Escrow {
    pub fn deadline(env: Env) -> u64 {
        env.ledger().timestamp() + 86400 // 1 day, in seconds
    }
}
```

## How Sanctifier detects it

The rule flags a binary arithmetic or comparison expression where one side
contains a `.sequence()` call and the other is an integer literal of
seconds magnitude (`>= 60`). Timestamp-based math and small ledger deltas are
intentionally left alone to keep the signal high; findings dedupe per source line.

**Limitations:** it is a syntactic heuristic keyed on the literal magnitude — a
seconds value stored in a variable, or a duration below the threshold, may be
missed.

## References

- [Soroban: `Ledger` — sequence vs. timestamp](https://soroban.stellar.org/docs)
- [CWE-682: Incorrect Calculation](https://cwe.mitre.org/data/definitions/682.html)
- Related: [`missing_ttl`](missing_ttl.md)
