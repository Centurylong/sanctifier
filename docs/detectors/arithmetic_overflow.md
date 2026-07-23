# `arithmetic_overflow` — Unchecked arithmetic

| | |
| --- | --- |
| **Finding code** | [`S003`](../error-codes.md) |
| **Category** | arithmetic |
| **Severity** | High |
| **Source rule** | [`rules/arithmetic_overflow.rs`](../../tooling/sanctifier-core/src/rules/arithmetic_overflow.rs) |
| **Glossary** | [Overflow / underflow](../glossary.md) |

## What it catches

Raw `+`, `-`, or `*` on integer types in a context where the operands are
attacker-influenced. In release builds Rust arithmetic **wraps** silently, so an
overflow can mint balances, zero out debts, or bypass supply caps without any
panic. Soroban's financial primitives (`i128` balances, `u64` timestamps) are
exactly the values an attacker wants to overflow.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    pub fn mint(env: Env, to: Address, amount: i128) {
        let mut supply: i128 = env.storage().instance().get(&SUPPLY).unwrap_or(0);
        supply = supply + amount; // wraps past i128::MAX → supply cap bypass
        env.storage().instance().set(&SUPPLY, &supply);
    }
}
```

## The fix

Use the checked API and convert overflow into a typed error:

```rust
#[contractimpl]
impl Token {
    pub fn mint(env: Env, to: Address, amount: i128) -> Result<(), Error> {
        let supply: i128 = env.storage().instance().get(&SUPPLY).unwrap_or(0);
        let new_supply = supply.checked_add(amount).ok_or(Error::Overflow)?;
        env.storage().instance().set(&SUPPLY, &new_supply);
        Ok(())
    }
}
```

`checked_add` / `checked_sub` / `checked_mul` return `None` on overflow; prefer
them over `wrapping_*` or `saturating_*` unless wrapping is the intended domain
behaviour.

## How Sanctifier detects it

An AST visitor inspects binary expressions (`ExprBinary`) using `+`/`-`/`*` and
compound-assign forms, skipping operations that are already `checked_*` /
`saturating_*` calls or that operate on obviously-constant operands. Each hit
reports the function, operation, and a suggested checked replacement.

**Limitations:** it is a syntactic heuristic, not a range analysis — it cannot
prove an operation is safe, so infallible arithmetic may need a suppression.

## References

- Soroban docs — [Built-in types](https://soroban.stellar.org/docs/fundamentals-and-concepts/built-in-types)
- [CWE-190: Integer Overflow or Wraparound](https://cwe.mitre.org/data/definitions/190.html)
- Related: [`fee_rounding`](fee_rounding.md), [`edge_amount`](edge_amount.md)
