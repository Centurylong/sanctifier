# `missing_ttl` — Storage access without TTL extension

| | |
| --- | --- |
| **Finding code** | [`S006`](../error-codes.md) |
| **Category** | storage_durability |
| **Severity** | Medium |
| **Source rule** | [`rules/missing_ttl.rs`](../../tooling/sanctifier-core/src/rules/missing_ttl.rs) |
| **Glossary** | [State archival / TTL](../glossary.md#state-archival--ttl) · [Persistent storage](../glossary.md#persistent-storage) |

## What it catches

A contract reads or writes **persistent** or **instance** storage but never
extends the entry's time-to-live (TTL). In Soroban, entries whose TTL lapses are
archived and become unreadable until restored. A contract that never bumps TTL
can have live balances or config silently archived out from under it, freezing
funds or bricking the instance.

## Vulnerable example

```rust
#[contractimpl]
impl Vault {
    pub fn deposit(env: Env, user: Address, amount: i128) {
        user.require_auth();
        let key = DataKey::Balance(user);
        let bal: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(bal + amount));
        // No extend_ttl: this balance can be archived and become unreadable.
    }
}
```

## The fix

Bump the TTL whenever you touch a long-lived entry:

```rust
const DAY_LEDGERS: u32 = 17_280;      // ~1 day of ledgers
const BUMP_AMOUNT: u32 = 30 * DAY_LEDGERS;
const LIFETIME_THRESHOLD: u32 = BUMP_AMOUNT - DAY_LEDGERS;

#[contractimpl]
impl Vault {
    pub fn deposit(env: Env, user: Address, amount: i128) {
        user.require_auth();
        let key = DataKey::Balance(user);
        let bal: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(bal + amount));
        env.storage()
            .persistent()
            .extend_ttl(&key, LIFETIME_THRESHOLD, BUMP_AMOUNT);
    }
}
```

## How Sanctifier detects it

The rule finds functions that access `storage().persistent()` or
`storage().instance()` and checks whether the same function also calls
`extend_ttl` (or the instance-level `extend_ttl`). Access without a matching
extension is reported.

**Limitations:** TTL bumped centrally in a shared helper, or amortized across
calls, can produce a false positive — suppress with a justification where the
archival policy is intentional.

## References

- Soroban docs — [State archival](https://soroban.stellar.org/docs/fundamentals-and-concepts/state-archival)
- [CWE-404: Improper Resource Shutdown or Release](https://cwe.mitre.org/data/definitions/404.html)
- Related: [`ledger_size`](ledger_size.md)
