# `ttl_extend_misconfig` - Misconfigured TTL extension window

| | |
| --- | --- |
| **Finding code** | [`SANCT_TTL_EXTEND_MISCONFIG`](../error-codes.md) |
| **Category** | storage_durability |
| **Severity** | Medium |
| **Source rule** | [`rules/ttl_extend_misconfig.rs`](../../tooling/sanctifier-core/src/rules/ttl_extend_misconfig.rs) |
| **Glossary** | [State archival / TTL](../glossary.md#state-archival--ttl) |

## What it catches

A Soroban storage call where static arithmetic proves that
`extend_ttl(threshold, extend_to)` has `threshold >= extend_to`.
That ordering is a TTL-policy misconfiguration because the renewal trigger is
not below the target lifetime.

The detector evaluates integer literals and checked constant arithmetic.
Runtime-dependent arguments are deliberately left unreported.

## Vulnerable example

```rust
env.storage()
    .persistent()
    .extend_ttl(&key, 30 * 17_280, 29 * 17_280);
```

## The fix

Keep the renewal trigger strictly below the target:

```rust
env.storage()
    .persistent()
    .extend_ttl(&key, 29 * 17_280, 30 * 17_280);
```

Persistent, instance, and temporary storage forms are recognized. Calls to an
unrelated method named `extend_ttl` are ignored because the receiver must be a
Soroban `env.storage()` chain.

## References

- Soroban state archival and TTL policy: [State archival](https://soroban.stellar.org/docs/fundamentals-and-concepts/state-archival)
- Related: [`missing_ttl`](missing_ttl.md)
