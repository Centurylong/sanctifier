# `migrate_auth` — Unauthenticated migration entrypoint

| | |
| --- | --- |
| **Finding code** | [`SANCT_MIGRATE_AUTH`](../error-codes.md) |
| **Category** | authentication |
| **Severity** | Error |
| **Source rule** | [`rules/migrate_auth.rs`](../../tooling/sanctifier-core/src/rules/migrate_auth.rs) |

## What it catches

A public function named `migrate` that has no visible authorization guard.
Migration entrypoints commonly rewrite contract storage or move data between
schema versions. If any caller can trigger that transition, they can mutate
privileged state, repeat a migration at an unsafe time, or corrupt the storage
layout the new code expects.

## Vulnerable example

```rust
pub struct Contract;

impl Contract {
    pub fn migrate(env: Env) {
        let legacy: i128 = env.storage().instance().get(&DataKey::Legacy).unwrap();
        env.storage().instance().set(&DataKey::Current, &legacy);
    }
}
```

The entrypoint is public and performs the migration without authenticating an
administrator first.

## The fix

Require authorization from the migration authority before changing state:

```rust
pub struct Contract;

impl Contract {
    pub fn migrate(env: Env, admin: Address) {
        admin.require_auth();
        let legacy: i128 = env.storage().instance().get(&DataKey::Legacy).unwrap();
        env.storage().instance().set(&DataKey::Current, &legacy);
    }
}
```

`require_auth_for_args(...)` is also recognized when the migration permission
is intentionally bound to specific arguments.

## How Sanctifier detects it

The rule inspects public free functions and public impl methods whose name is
exactly `migrate`. It accepts `require_auth()` or `require_auth_for_args()`
only when the method receiver resolves to an `Address`-typed parameter. The
validated UFCS form `Address::require_auth(&admin)` is also recognized when its
argument resolves to such a parameter. Macro and free-function lookalikes are
not treated as authorization guards. A public `migrate` with no structurally
bound guard emits `SANCT_MIGRATE_AUTH`.

**Limitations:** this is an intraprocedural structural check. It does not prove
that the authenticated address is the correct administrator, that the guard
runs before every state change, or that an authorization helper called from
another function is equivalent to `require_auth`. Those cases still require
review.

## References

- [Issue #675 — access control missing on migrate()](https://github.com/Centurylong/sanctifier/issues/675)
- [Stellar authorization documentation](https://developers.stellar.org/docs/learn/fundamentals/contract-development/authorization)
- [CWE-862: Missing Authorization](https://cwe.mitre.org/data/definitions/862.html)
