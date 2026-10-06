# `loop_accumulator` — Unchecked accumulation inside a loop

| | |
| --- | --- |
| **Finding code** | [`SANCT_LOOP_ACCUMULATOR`](../error-codes.md) |
| **Category** | arithmetic |
| **Severity** | Warning |
| **Source rule** | [`rules/loop_accumulator.rs`](../../tooling/sanctifier-core/src/rules/loop_accumulator.rs) |
| **Glossary** | [Overflow / underflow](../glossary.md) |

## What it catches

A variable that survives successive loop iterations and is updated with
unchecked addition, such as `total += amount` or `total = total + amount`.
The individual amounts can fit in the variable's integer type while their
running total exceeds its range. Depending on overflow-check settings, this
can abort execution or wrap the accumulated value.

The rule covers `for`, `while`, and `loop`, including additions in a `while`
condition. It also recognizes the accumulator on the right of `+` and in
addition chains such as `total = total + amount + fee`.

## Vulnerable example

```rust
#[contractimpl]
impl Payouts {
    pub fn total_amounts(amounts: Vec<i128>) -> i128 {
        let mut total: i128 = 0;
        for amount in amounts.iter() {
            total += amount;
        }
        total
    }
}
```

## The fix

Use `checked_add` and handle overflow explicitly, for example by returning
the contract's overflow error:

```rust
#[contractimpl]
impl Payouts {
    pub fn total_amounts(amounts: Vec<i128>) -> Result<i128, Error> {
        let mut total: i128 = 0;
        for amount in amounts.iter() {
            total = total.checked_add(amount).ok_or(Error::Overflow)?;
        }
        Ok(total)
    }
}
```

Use `total = total.saturating_add(amount)` when clamping to the integer
type's boundary is the intended behavior. Saturation changes the result;
it is not a substitute for rejecting an invalid financial total.

## How Sanctifier detects it

An AST visitor tracks lexical bindings and loop nesting. It reports an
unchecked `+=` or additive assignment that reads the same accumulator,
provided that binding was introduced before an enclosing loop. An additive
chain produces one finding for its assignment.

A temporary declared afresh inside a loop, including a loop-pattern binding
or a shadow of an outer variable, is not treated as an accumulator for that
loop. A subtotal declared in an outer loop and updated by an inner loop is
still reported. The iterator expression of a `for` loop is visited before
entering that loop, and deferred closure and async bodies do not inherit
their surrounding loop context.

Calls to `checked_add` and `saturating_add` do not match the unchecked-addition
patterns. Using a checked operation elsewhere does not make an unchecked
accumulator addition safe.

**Limitations:** this is a syntactic heuristic, not type or range analysis.
It does not prove that a loop can overflow, account for an outer accumulator
reset on every iteration, or follow aliases and helper calls. A fixed loop
bound alone does not suppress the finding. Ordinary additions that do not
update an accumulator are outside this rule's scope; see
[`arithmetic_overflow`](arithmetic_overflow.md) for broader arithmetic checks.

## References

- Related: [`arithmetic_overflow`](arithmetic_overflow.md), [`unsigned_underflow`](unsigned_underflow.md)
