# `unbounded_storage` — Storage collection grows without a bound

| | |
| --- | --- |
| **Finding code** | [`SANCT_UNBOUNDED_STORAGE`](../error-codes.md) |
| **Category** | denial_of_service |
| **Severity** | High |
| **Source rule** | [`rules/unbounded_storage.rs`](../../tooling/sanctifier-core/src/rules/unbounded_storage.rs) |
| **Glossary** | [Ledger entry](../glossary.md#ledger-entry) · [State bloat](../glossary.md#state-bloat) |

## What it catches

A persistent or instance storage collection (`Vec` / `Map`) that is read, **grown**
via `push_back` / `insert` / `set`, and written back — with **no removal path and no
length cap**. Every call adds an entry that is never reclaimed, so the collection
grows without bound. Eventually the entry exceeds the ledger size limit and the
contract's own writes start to fail, bricking the affected path — an attacker can
force this cheaply by calling the growth entrypoint in a loop.

This is the storage sibling of [`arg_dos`](arg_dos.md): `arg_dos` flags unbounded
iteration over a *call argument*; `unbounded_storage` flags unbounded growth of
*persisted state*.

## Vulnerable example

```rust
#[contractimpl]
impl Registry {
    // Append-only persistent Vec: every registration grows `members`, and
    // nothing ever removes an entry or caps the length.
    pub fn register(env: Env, who: Address) {
        let key = Symbol::new(&env, "members");
        let mut members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        members.push_back(who);
        env.storage().persistent().set(&key, &members);
    }
}
```

## The fix

Bound the growth. Either cap the length before appending, or store per-key entries
instead of one ever-growing collection so individual items can be pruned:

```rust
#[contractimpl]
impl Registry {
    pub fn register_capped(env: Env, who: Address) {
        let key = Symbol::new(&env, "members");
        let mut members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        // Explicit cap: refuse to grow past a known-safe bound.
        assert!(members.len() < 1000, "member list is full");
        members.push_back(who);
        env.storage().persistent().set(&key, &members);
    }

    // Or: key each member individually so entries stay small and removable.
    pub fn register_keyed(env: Env, who: Address) {
        env.storage().persistent().set(&DataKey::Member(who), &true);
    }
}
```

## How Sanctifier detects it

The rule walks entrypoints for the read → grow → write-back pattern on a
`storage().persistent()` / `instance()` collection and flags it when it sees no
guarding length check and no corresponding removal (`remove` / `pop` / a `len()`
comparison) on the same key.

**Limitations:** it reasons about a single function. A collection that is bounded by
logic in another function, or pruned on a schedule the rule can't see, is a false
positive — add an explicit `len()` guard or suppress with a justification. It also
does not size-estimate the entry; pair it with [`ledger_size`](ledger_size.md).

## References

- Soroban docs — [Persisting Data](https://soroban.stellar.org/docs/fundamentals-and-concepts/persisting-data)
- [CWE-770: Allocation of Resources Without Limits or Throttling](https://cwe.mitre.org/data/definitions/770.html)
- Related: [`arg_dos`](arg_dos.md), [`ledger_size`](ledger_size.md)
