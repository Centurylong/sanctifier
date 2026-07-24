# `panic_detection` — Panic / unwrap / expect usage

| | |
| --- | --- |
| **Finding code** | [`S002`](../error-codes.md) |
| **Category** | panic_handling |
| **Severity** | High |
| **Source rule** | [`rules/panic_detection.rs`](../../tooling/sanctifier-core/src/rules/panic_detection.rs) |
| **Glossary** | [Panic](../glossary.md#panic) · [`unwrap` / `expect`](../glossary.md#unwrap--expect) |

## What it catches

Use of `panic!`, `unwrap()`, or `expect()` anywhere in contract code. In a
Soroban contract a panic **traps the entire invocation** and rolls back the
transaction with an opaque error. An attacker who can steer input into a panicking
path gains a cheap denial-of-service, and honest users get an undebuggable
failure instead of a typed contract error.

See also [`sanct_unwrap`](sanct_unwrap.md), which scopes the same idea tightly to
`#[contractimpl]` entrypoints.

## Vulnerable example

```rust
#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, user: Address) -> i128 {
        // Traps if the user has no balance entry — a griefer can force reverts.
        let balance: i128 = env.storage().persistent().get(&user).unwrap();
        balance
    }
}
```

## The fix

Return a typed error or an explicit default instead of trapping:

```rust
#[contracterror]
#[derive(Copy, Clone)]
pub enum Error {
    NoBalance = 1,
}

#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, user: Address) -> Result<i128, Error> {
        env.storage()
            .persistent()
            .get(&user)
            .ok_or(Error::NoBalance)
    }
}
```

## How Sanctifier detects it

The rule walks the AST for `panic!` macro invocations and method calls named
`unwrap` / `expect`, reporting the enclosing function and location. Test modules
(`#[cfg(test)]`) are treated the same as production code, so guard test-only
panics with an inline suppression if needed.

**Limitations:** it does not prove a path is reachable; it flags the syntactic
pattern. Legitimately-infallible unwraps should be rewritten or suppressed with a
justification.

## References

- Soroban docs — [Errors](https://soroban.stellar.org/docs/fundamentals-and-concepts/errors-and-panics)
- [CWE-248: Uncaught Exception](https://cwe.mitre.org/data/definitions/248.html)
- Related: [`sanct_unwrap`](sanct_unwrap.md), [`unhandled_result`](unhandled_result.md)
