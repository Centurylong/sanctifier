# `edge_amount` — Missing amount / self-transfer guards

| | |
| --- | --- |
| **Finding code** | [`S013`](../error-codes.md) |
| **Category** | code_hygiene |
| **Severity** | Medium |
| **Source rule** | [`rules/edge_amount.rs`](../../tooling/sanctifier-core/src/rules/edge_amount.rs) |
| **Glossary** | [Address](../glossary.md#address) |

## What it catches

A `transfer` / `mint` / `burn`-style function that never validates `amount > 0`
or that `from != to`. Zero/negative amounts can emit misleading events or trip
downstream accounting; self-transfers (`from == to`) can, with a naive
read-modify-write, **duplicate balance** by reading the pre-image twice.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        // No amount check, no from != to check.
        let from_bal: i128 = balance(&env, &from);
        let to_bal: i128 = balance(&env, &to);          // if to == from, stale read
        set_balance(&env, &from, from_bal - amount);
        set_balance(&env, &to, to_bal + amount);         // overwrites the debit
    }
}
```

## The fix

Reject edge inputs up front:

```rust
#[contractimpl]
impl Token {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if from == to {
            return Ok(()); // or Err(Error::SelfTransfer) — but never double-count
        }
        // ... safe read-modify-write ...
        Ok(())
    }
}
```

## How Sanctifier detects it

The rule matches functions whose names/signatures look like value movement
(`transfer`, `mint`, `burn`, an `amount: i128` parameter) and checks the body for
a guard comparing `amount` against zero and, when two `Address` parameters are
present, a `from != to` comparison. Missing guards are reported.

**Limitations:** guards implemented via a shared validation helper may not be
recognized; suppress with a justification when validation is centralized.

## References

- Soroban docs — [Tokens / SEP-41](https://soroban.stellar.org/docs/tokens/token-interface)
- [CWE-20: Improper Input Validation](https://cwe.mitre.org/data/definitions/20.html)
- Related: [`arithmetic_overflow`](arithmetic_overflow.md), [`auth_gap`](auth_gap.md)
