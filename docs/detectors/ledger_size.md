# `ledger_size` — Ledger entry size risk

| | |
| --- | --- |
| **Finding code** | [`S004`](../error-codes.md) |
| **Category** | storage_limits |
| **Severity** | Medium |
| **Source rule** | [`rules/ledger_size.rs`](../../tooling/sanctifier-core/src/rules/ledger_size.rs) |
| **Glossary** | [Ledger entry size limit](../glossary.md#ledger-entry-size-limit) · [OOG](../glossary.md#oog-out-of-gas) |

## What it catches

A `#[contracttype]` struct or enum whose estimated serialized size approaches or
exceeds the ledger entry size limit (~64 KB by default, configurable with
`--limit`). Oversized entries are expensive to read/write and can make an
entrypoint **impossible to call** once the data grows, effectively bricking the
contract for that key.

## Vulnerable example

```rust
#[contracttype]
pub struct Registry {
    // An unbounded Vec inside a single ledger entry: grows until writes fail.
    pub entries: Vec<Record>,   // Record is itself large
    pub audit_log: Vec<String>, // append-only, never pruned
}
```

## The fix

Shard large collections across multiple keys so no single entry is unbounded:

```rust
#[contracttype]
pub enum DataKey {
    Record(u32),      // one ledger entry per record
    RecordCount,      // small counter entry
}

// Store each record under its own key instead of one giant Vec.
env.storage().persistent().set(&DataKey::Record(id), &record);
```

Keep hot, frequently-read metadata small; move append-only history off-chain or
into per-item keys.

## How Sanctifier detects it

The rule parses `#[contracttype]` definitions, estimates a serialized size from
field types (recursively, with conservative sizes for `Vec`/`Map`/`String`/
`Bytes`), and emits `ApproachingLimit` or `ExceedsLimit` when the estimate
crosses the configured thresholds.

**Limitations:** the estimate is static and cannot know runtime collection
lengths; it assumes conservative bounds for dynamically-sized fields.

## References

- Soroban docs — [Persisting data / state archival](https://soroban.stellar.org/docs/fundamentals-and-concepts/persisting-data)
- [CWE-770: Allocation of Resources Without Limits](https://cwe.mitre.org/data/definitions/770.html)
- Related: [`missing_ttl`](missing_ttl.md), [`arg_dos`](arg_dos.md)
