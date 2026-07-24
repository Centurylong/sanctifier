# `fee_rounding` — Integer-division fee rounds to zero

| | |
| --- | --- |
| **Finding code** | [`S017`](../error-codes.md) |
| **Category** | arithmetic |
| **Severity** | High |
| **Source rule** | [`rules/fee_rounding.rs`](../../tooling/sanctifier-core/src/rules/fee_rounding.rs) |
| **Glossary** | [Overflow / underflow](../glossary.md) |

## What it catches

A fee or interest calculation of the form `amount * rate / denominator` (or
`amount / denominator`) where integer truncation makes the result **zero** for
small `amount`. An attacker splits one large action into many micro-transactions,
each rounding the fee down to `0`, and evades the fee entirely while still moving
the full value.

## Vulnerable example

```rust
#[contractimpl]
impl Amm {
    // fee = amount * 30 / 10_000  (0.3%). For amount < 34, fee rounds to 0.
    pub fn swap(env: Env, user: Address, amount: i128) -> i128 {
        user.require_auth();
        let fee = amount * 30 / 10_000; // micro-swaps pay no fee
        let out = amount - fee;
        settle(&env, &user, out, fee);
        out
    }
}
```

## The fix

Round fees **up**, or enforce a minimum fee / minimum trade size:

```rust
#[contractimpl]
impl Amm {
    pub fn swap(env: Env, user: Address, amount: i128) -> Result<i128, Error> {
        user.require_auth();
        // Ceiling division: (a * r + (d - 1)) / d, with checked arithmetic.
        let num = amount.checked_mul(30).ok_or(Error::Overflow)?;
        let fee = num.checked_add(9_999).ok_or(Error::Overflow)? / 10_000;
        if fee == 0 {
            return Err(Error::AmountTooSmall);
        }
        let out = amount - fee;
        settle(&env, &user, out, fee);
        Ok(out)
    }
}
```

## How Sanctifier detects it

The rule looks for division expressions whose numerator is a `amount * rate`
product (or a bare `amount`) in a fee/interest context, where the denominator is
a basis-points-like constant, and flags them as round-to-zero-prone. It suggests
ceiling division or a minimum-amount guard.

**Limitations:** it is a syntactic heuristic keyed on shape and constant
denominators; fee math split across helpers may be missed.

## References

- [CWE-682: Incorrect Calculation](https://cwe.mitre.org/data/definitions/682.html)
- [CWE-1339: Insufficient Precision or Accuracy of a Real Number](https://cwe.mitre.org/data/definitions/1339.html)
- Related: [`arithmetic_overflow`](arithmetic_overflow.md), [`edge_amount`](edge_amount.md)
