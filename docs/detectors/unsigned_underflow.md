# `unsigned_underflow` — Unchecked subtraction underflows on unsigned integers

| | |
| --- | --- |
| **Finding code** | [`S019`](../error-codes.md) |
| **Category** | arithmetic |
| **Severity** | High |
| **Source rule** | [`rules/unsigned_underflow.rs`](../../tooling/sanctifier-core/src/rules/unsigned_underflow.rs) |
| **Glossary** | [Overflow / underflow](../glossary.md) |

## What it catches

Bare subtraction — `a - b` or `a -= b` — whose left operand is an **unsigned**
integer (`u8`/`u16`/`u32`/`u64`/`u128`/`usize`). On Soroban/WASM release builds
these operations *wrap*, so subtracting past zero silently produces a huge value
instead of trapping. The classic case is `balance - amount` when
`amount > balance`: the account's balance appears to become enormous.

## Vulnerable example

```rust
#[contractimpl]
impl Ledger {
    // If amount > balance, `balance - amount` wraps to a near-u64::MAX value.
    pub fn withdraw(env: Env, balance: u64, amount: u64) -> u64 {
        balance - amount
    }
}
```

## The fix

Use `checked_sub` and surface the shortfall as a typed error, or `saturating_sub`
when clamping to zero is the intended behaviour:

```rust
#[contractimpl]
impl Ledger {
    pub fn withdraw(env: Env, balance: u64, amount: u64) -> Result<u64, Error> {
        balance.checked_sub(amount).ok_or(Error::InsufficientBalance)
    }
}
```

## How Sanctifier detects it

The rule collects the unsigned-typed bindings in each function (unsigned
parameters plus locals with an explicit unsigned type annotation), then flags any
bare `-` / `-=` whose left operand is one of them. Signed arithmetic and the
`checked_sub` / `saturating_sub` method forms are intentionally left alone to keep
the signal high; findings dedupe per source line.

**Limitations:** it is unsigned-specific and syntactic — subtraction whose type is
only known after inference (e.g. via a return type or generic) may be missed.

## References

- [CWE-191: Integer Underflow](https://cwe.mitre.org/data/definitions/191.html)
- [Soroban: integers and overflow](https://soroban.stellar.org/docs)
- Related: [`arithmetic_overflow`](arithmetic_overflow.md), [`fee_rounding`](fee_rounding.md)
