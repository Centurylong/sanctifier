# `balance_equality` — Balance gated with `==` / `!=` instead of `>=` / `<=`

| | |
| --- | --- |
| **Finding code** | [`SANCT_BALANCE_EQ`](../error-codes.md) |
| **Category** | logic |
| **Severity** | Info |
| **Source rule** | [`rules/balance_equality.rs`](../../tooling/sanctifier-core/src/rules/balance_equality.rs) |
| **Glossary** | [Balance](../glossary.md#balance) |

## What it catches

A spend/withdraw guard that compares a **balance** against an **amount** with
exact (in)equality — `balance == amount` or `balance != amount` — where a
threshold comparison (`>=` / `<=`) was almost certainly intended. Exact equality
is only true on one precise value, so the guard either **locks funds** (the
happy path is reachable only when the balance is *exactly* the amount) or invites
edge-case exploitation. The detector classifies each operand by name (balance /
reserve / supply / vault … vs amount / withdraw / payment …) and only fires when
one side is a balance and the other an amount, keeping false positives low.

## Vulnerable example

```rust
#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, user: Address, amount: i128) {
        user.require_auth();
        let balance = get_balance(&env, &user);
        // Only lets the withdrawal through when the balance is *exactly*
        // the amount — any other balance (including a larger one) is rejected.
        if balance == amount {
            do_withdraw(&env, &user, amount);
        }
    }
}
```

## The fix

Gate on a threshold, not on exact equality:

```rust
#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, user: Address, amount: i128) {
        user.require_auth();
        let balance = get_balance(&env, &user);
        if balance >= amount {
            do_withdraw(&env, &user, amount);
        }
    }
}
```

## How Sanctifier detects it

A `syn` AST visitor inspects every binary `==` / `!=` expression, extracts a name
for each operand (path, field, method-call, or call target), and classifies it as
a balance, an amount, or neither. It reports the comparison only when the two
sides classify as a (balance, amount) pair. Because the signal is name-based, it
is advisory (**Info**): rename-heavy code or unconventional identifiers may fall
outside the heuristic, and a genuine exact-equality check between a balance and an
amount is rare but not impossible.

## References

- [Soroban token interface](https://developers.stellar.org/docs/tokens/token-interface) — balance/spend semantics.
- [CWE-697: Incorrect Comparison](https://cwe.mitre.org/data/definitions/697.html)
- Related: [`edge_amount`](edge_amount.md), [`arithmetic_overflow`](arithmetic_overflow.md).
