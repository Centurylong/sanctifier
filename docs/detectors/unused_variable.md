# `unused_variable` — Unused local binding (dead code)

| | |
| --- | --- |
| **Finding code** | [`S015`](../error-codes.md) |
| **Category** | code_hygiene |
| **Severity** | Info |
| **Source rule** | [`rules/unused_variable.rs`](../../tooling/sanctifier-core/src/rules/unused_variable.rs) |
| **Glossary** | [Dead code](../glossary.md) |

## What it catches

A local variable that is bound but never read. On its own this is a hygiene
issue, but in contract code an unused binding is often the **symptom of a
dropped check**: a computed authorization result, a validated amount, or an error
value that was meant to be used and silently isn't.

## Vulnerable example

```rust
#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, user: Address, amount: i128) {
        user.require_auth();
        // `allowed` is computed but never checked — the guard is dead.
        let allowed = amount <= max_withdrawal(&env, &user);
        pay(&env, &user, amount);
    }
}
```

## The fix

Either use the binding (restore the intended check) or delete it:

```rust
#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        if amount > max_withdrawal(&env, &user) {
            return Err(Error::LimitExceeded);
        }
        pay(&env, &user, amount);
        Ok(())
    }
}
```

If a binding is intentionally unused, prefix it with `_` to document that.

## How Sanctifier detects it

The rule collects `let` bindings in each function and checks whether each
identifier is referenced later in the same scope, ignoring names that start with
`_`. Unreferenced bindings are reported with a fix suggestion.

**Limitations:** it is scope-local and does not track uses through macros; a
binding consumed only inside a macro expansion may be falsely flagged.

## References

- Rust — [Unused variables lint](https://doc.rust-lang.org/rustc/lints/listing/warn-by-default.html#unused-variables)
- [CWE-563: Assignment to Variable without Use](https://cwe.mitre.org/data/definitions/563.html)
- Related: [`unhandled_result`](unhandled_result.md)
