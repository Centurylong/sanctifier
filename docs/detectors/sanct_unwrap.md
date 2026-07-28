# `sanct_unwrap` — Risky unwrap in an entrypoint

| | |
| --- | --- |
| **Finding code** | [`SANCT_UNWRAP`](../error-codes.md) |
| **Category** | panic_handling |
| **Severity** | High |
| **Source rule** | [`rules/sanct_unwrap.rs`](../../tooling/sanctifier-core/src/rules/sanct_unwrap.rs) |
| **Glossary** | [`unwrap` / `expect`](../glossary.md#unwrap--expect) · [Panic](../glossary.md#panic) |

## What it catches

`unwrap()`, `expect(..)`, or a risky `unwrap_or_default()` **inside a
`#[contractimpl]` entrypoint**. This is the high-signal subset of
[`panic_detection`](panic_detection.md): in an entrypoint, an attacker-triggered
missing value either aborts the whole transaction (`unwrap`/`expect`) or silently
turns absent financial state into a default (`unwrap_or_default` yielding `0`
balances, empty maps, or the zero address).

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    pub fn balance(env: Env, id: Address) -> i128 {
        // If the entry is missing, this silently returns 0 — indistinguishable
        // from a real zero balance, and hides storage/archival bugs.
        env.storage()
            .persistent()
            .get(&DataKey::Balance(id))
            .unwrap_or_default()
    }
}
```

## The fix

Return a typed `Result`, or map missing state to an explicit, intended default:

```rust
#[contractimpl]
impl Token {
    pub fn balance(env: Env, id: Address) -> Result<i128, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(id))
            .ok_or(Error::AccountNotFound)
    }
}
```

Use `unwrap_or(0)` only where zero is the deliberate contract semantics for an
absent entry — and say so in a comment.

## How Sanctifier detects it

The rule walks methods inside `#[contractimpl]` blocks and flags `unwrap` /
`expect` / `unwrap_or_default` calls, reporting the entrypoint and location. It is
intentionally scoped to entrypoints to keep the signal high; whole-crate
`unwrap` usage is covered by [`panic_detection`](panic_detection.md).

**Limitations:** it does not distinguish a provably-infallible `unwrap` from a
dangerous one — rewrite or suppress with a justification.

## References

- Soroban docs — [Errors](https://soroban.stellar.org/docs/fundamentals-and-concepts/errors-and-panics)
- [CWE-248: Uncaught Exception](https://cwe.mitre.org/data/definitions/248.html)
- Related: [`panic_detection`](panic_detection.md), [`unhandled_result`](unhandled_result.md)
