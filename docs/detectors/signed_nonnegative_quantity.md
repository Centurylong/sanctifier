# `signed_nonnegative_quantity` — Signed non-negative quantities

| | |
| --- | --- |
| **Finding code** | [`SANCT_SIGNED_QUANTITY`](../error-codes.md) |
| **Category** | arithmetic |
| **Severity** | Info |
| **Source rule** | [`rules/signed_nonnegative_quantity.rs`](../../tooling/sanctifier-core/src/rules/signed_nonnegative_quantity.rs) |

## What it catches

Signed primitive integer fields or function parameters whose names clearly describe
balances or amounts even though negative values are not part of the intended
domain. Keeping a non-negative quantity in a signed type makes negative-state
bugs representable and can hide missing input validation.

The detector is deliberately narrow: it looks for `amount` / `balance`
name components, ignores delta/diff/change/offset/net semantics, ignores unsigned
integer types, and treats an explicit non-negative guard on a parameter as
sufficient evidence that the signed representation is intentional.

## Vulnerable example

```rust
struct VaultState {
    balance: i128,
}

fn withdraw(amount: i128) {
    consume(amount);
}
```

Both values represent non-negative quantities, but the type admits negative
values and `withdraw` does not reject them.

## The fix

Prefer an unsigned type when negative values are impossible by design:

```rust
struct VaultState {
    balance: u128,
}
```

When an API must stay signed, reject negatives before the value is used:

```rust
fn withdraw(amount: i128) -> Result<(), Error> {
    if amount < 0 {
        return Err(Error::InvalidAmount);
    }
    consume(amount);
    Ok(())
}
```

## How Sanctifier detects it

The rule inspects signed primitive integer struct fields and function parameters.
A name must contain an `amount` or `balance` component. Parameters are
suppressed when the function contains an explicit `>= 0` assertion/require
or a terminating `if value < 0 { ... }` rejection. Struct fields remain an
advisory because their declared representation itself permits negative state.

**Limitations:** this is a naming-based advisory, not whole-program range
analysis. Domain-specific names that do not contain the recognized components
are not inferred, and validation delegated entirely to a helper may not be
recognized.

## References

- [Rust primitive integer types](https://doc.rust-lang.org/reference/types/numeric.html)
- [CWE-20: Improper Input Validation](https://cwe.mitre.org/data/definitions/20.html)
- Related: [`edge_amount`](edge_amount.md), [`unsigned_underflow`](unsigned_underflow.md)
