# `division_by_zero` — Division or modulo by a possibly-zero value

| | |
| --- | --- |
| **Finding code** | [`S018`](../error-codes.md) |
| **Category** | arithmetic |
| **Severity** | Medium |
| **Source rule** | [`rules/division_by_zero.rs`](../../tooling/sanctifier-core/src/rules/division_by_zero.rs) |
| **Glossary** | [Host panic](../glossary.md#host-panic) · [Denominator](../glossary.md#denominator) |

## What it catches

A `/` or `%` where the denominator is a **non-constant value that has not been
proven non-zero** on the path to the operation. On Soroban's host, integer
division or remainder by zero traps and aborts the invocation, so a
caller-supplied or state-derived zero turns a plain calculation into a
denial-of-service — the entrypoint panics instead of returning.

Constant denominators (`amount / 10_000`) are safe and are not flagged, and a
denominator guarded by a prior `== 0` early return, a `!= 0` branch, or a
`checked_div` / `checked_rem` is treated as proven non-zero.

## Vulnerable example

```rust
#[contractimpl]
impl Vault {
    // `count` comes straight from the caller and is never checked, so
    // `average(total, 0)` panics on-chain instead of returning.
    pub fn average(env: Env, total: i128, count: i128) -> i128 {
        total / count
    }

    // Modulo by a possibly-zero argument aborts the same way.
    pub fn pick_winner(env: Env, seed: u64, players: u32) -> u32 {
        (seed as u32) % players
    }
}
```

## The fix

Prove the denominator is non-zero before dividing — an early-return guard, a
branch, or a non-panicking checked operation all work:

```rust
#[contractimpl]
impl Vault {
    // Early-return guard: the division is unreachable when `count == 0`.
    pub fn average_guarded(env: Env, total: i128, count: i128) -> i128 {
        if count == 0 {
            return 0;
        }
        total / count
    }

    // Or use the checked variant and handle the None case explicitly.
    pub fn average_checked(env: Env, total: i128, count: i128) -> i128 {
        total.checked_div(count).unwrap_or(0)
    }
}
```

## How Sanctifier detects it

The rule parses each entrypoint and walks its expressions for `/` and `%` binary
operations. A denominator that is an integer literal is ignored. For a variable
denominator it looks for a dominating guard in the same function — an
`if x == 0 { return … }` / `panic!` early exit, an enclosing `if x != 0 { … }`
branch, or use through `checked_div` / `checked_rem` — and only reports the
operation when no such guard proves the value non-zero.

**Limitations:** it reasons about a single function and simple syntactic guards.
A denominator bounded by logic in another function, or by an invariant the rule
can't see, is a false positive — add an explicit check or suppress it with a
justification. It does not perform range analysis on arithmetic that produces the
denominator.

## References

- Soroban docs — [Errors and panics](https://soroban.stellar.org/docs/fundamentals-and-concepts/errors-and-debugging)
- [CWE-369: Divide By Zero](https://cwe.mitre.org/data/definitions/369.html)
- Related: [`arithmetic_overflow`](arithmetic_overflow.md), [`fee_rounding`](fee_rounding.md)
