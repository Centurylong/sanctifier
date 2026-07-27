# `excessive_clone` — Gas-wasting clone of the Env handle

| | |
| --- | --- |
| **Finding code** | [`S020`](../error-codes.md) |
| **Category** | gas_efficiency |
| **Severity** | Low |
| **Source rule** | [`rules/excessive_clone.rs`](../../tooling/sanctifier-core/src/rules/excessive_clone.rs) |
| **Glossary** | [Gas / resource metering](../glossary.md) |

## What it catches

A `.clone()` of the Soroban `Env` handle — `env.clone()` or `self.env.clone()`.
The `Env` is a cheap-to-pass host handle, and cloning it is a common copy-paste
habit that adds needless host work on every invocation. Idiomatic Soroban passes
`&env` by reference (or accepts `&Env` in the callee) rather than handing around
owned clones.

## Vulnerable example

```rust
#[contractimpl]
impl Registry {
    pub fn record(env: Env, who: Address) {
        helper(env.clone(), who); // clones the host handle on every call
    }
}
```

## The fix

Borrow the `Env` instead of cloning it:

```rust
#[contractimpl]
impl Registry {
    pub fn record(env: Env, who: Address) {
        helper(&env, who);
    }
}

fn helper(env: &Env, who: Address) { /* ... */ }
```

## How Sanctifier detects it

The rule flags `.clone()` method calls whose receiver is the `Env` handle
(`env` or `self.env`). Ordinary domain-value clones (e.g. `Address`) are
intentionally left alone to keep the signal high; findings dedupe per source line.

**Limitations:** it targets the `Env` handle by name and is syntactic — an `Env`
bound to a differently named variable, or clones reached through a helper, may be
missed.

## References

- [Soroban: fees and metering](https://soroban.stellar.org/docs/fundamentals-and-concepts/fees-and-metering)
- [Rust: `Clone`](https://doc.rust-lang.org/std/clone/trait.Clone.html)
- Related: [`unbounded_storage`](unbounded_storage.md)
