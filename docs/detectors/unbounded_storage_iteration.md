# `unbounded_storage_iteration` — Unbounded iteration over durable storage

| | |
| --- | --- |
| **Finding code** | [`SANCT_UNBOUNDED_STORAGE_ITERATION`](../error-codes.md) |
| **Category** | denial_of_service |
| **Severity** | Warning |
| **Source rule** | [`rules/unbounded_storage_iteration.rs`](../../tooling/sanctifier-core/src/rules/unbounded_storage_iteration.rs) |
| **Related** | [`arg_dos`](arg_dos.md) · [`unbounded_storage`](unbounded_storage.md) |

## What it catches

A public Soroban contract entrypoint that loads a collection from
`storage().persistent()` or `storage().instance()` and then iterates the
whole collection without an obvious length cap or bounded iterator step.
Because durable state can grow independently of a single call, the loop's
worst-case work is O(n) in stored collection length and can eventually exhaust
the transaction budget.

## Vulnerable example

```rust
#[contractimpl]
impl Registry {
    pub fn rebuild_index(env: Env) {
        let members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&MEMBERS)
            .unwrap_or(Vec::new(&env));

        for member in members.iter() {
            rebuild_one(&env, member);
        }
    }
}
```

## The fix

Bound the work per invocation. Reject oversized state before the loop, or
process a bounded page and resume in a later invocation:

```rust
assert!(members.len() <= MAX_MEMBERS);
for member in members.iter() {
    rebuild_one(&env, member);
}

// Or process at most MAX_PER_CALL entries this invocation.
for member in members.iter().take(MAX_PER_CALL) {
    rebuild_one(&env, member);
}
```

## How Sanctifier detects it

The rule works within one public `#[contractimpl]` function. It records local
bindings whose initializer contains a `persistent().get(..)`,
`persistent().try_get(..)`, `instance().get(..)`, or
`instance().try_get(..)`, then looks for `for` loops over those bindings.
A dominating runtime upper bound suppresses the advisory: an enclosing
`if`/`while` condition that constrains `.len()`, an `assert!`/`assert_eq!`/
`ensure!`/`require!` guard that constrains the length, or an explicit `.take(..)`
iterator bound. A no-op bound such as `.take(collection.len())` does not suppress
the finding because it still permits O(n) work over the full stored collection.
Incidental length checks, `!=` tests, and `debug_assert!` do not count as
production bounds.

**Limitations:** this is intentionally local and conservative. Bounds hidden in
a helper function or enforced by a contract-wide invariant are not inferred.
If the invariant is real, make the bound explicit near the loop or use a
justified suppression.

## References

- Soroban docs — [Persisting Data](https://soroban.stellar.org/docs/fundamentals-and-concepts/persisting-data)
- [CWE-400: Uncontrolled Resource Consumption](https://cwe.mitre.org/data/definitions/400.html)
- Related: [`arg_dos`](arg_dos.md), [`unbounded_storage`](unbounded_storage.md)
