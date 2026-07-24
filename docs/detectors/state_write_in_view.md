# `state_write_in_view` — Getter/view function writes state

| | |
| --- | --- |
| **Finding code** | [`SANCT_STATE_WRITE_IN_VIEW`](../error-codes.md) |
| **Category** | code_hygiene |
| **Severity** | Warning |
| **Source rule** | [`rules/state_write_in_view.rs`](../../tooling/sanctifier-core/src/rules/state_write_in_view.rs) |
| **Glossary** | [Ledger entry](../glossary.md#ledger-entry) · [Persistent storage](../glossary.md#persistent-storage) |

## What it catches

A public function whose name reads as a **getter/view** (e.g. `get_*`, `*_of`,
`balance_of`, `allowance_of`, `view_*`) that nonetheless performs a storage
**write** — `set`, `update`, or `remove`. Callers, indexers, dashboards, and
off-chain consumers reasonably assume a getter is read-only and may call it
speculatively, cache it, or run it against a pinned ledger. A hidden write there
mutates state unexpectedly, costs the caller fees they didn't anticipate, and can
diverge on-chain state from what read-only consumers believe.

TTL bumps (`extend_ttl` and friends) are **never** flagged — extending the
lifetime of an entry you are reading is expected inside a getter and does not
change logical state.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    // Named like a read, but writes: it lazily initialises the balance on read.
    pub fn get_balance(env: Env, who: Address) -> i128 {
        let key = DataKey::Balance(who.clone());
        if !env.storage().persistent().has(&key) {
            env.storage().persistent().set(&key, &0i128); // ← hidden write
        }
        env.storage().persistent().get(&key).unwrap()
    }
}
```

## The fix

Keep getters pure. Do the initialisation in a dedicated state-changing function,
or fall back to a default without persisting it:

```rust
#[contractimpl]
impl Token {
    // Pure read: no write, defaults handled in-memory.
    pub fn get_balance(env: Env, who: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(who))
            .unwrap_or(0i128)
    }

    // Writes live in an explicitly state-changing entrypoint.
    pub fn init_account(env: Env, who: Address) {
        who.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Balance(who), &0i128);
    }
}
```

If a write inside a getter is genuinely intentional, rename the function so it no
longer reads as a view, or annotate the line:

```rust
env.storage().persistent().set(&key, &0i128); // sanctifier:ignore[SANCT_STATE_WRITE_IN_VIEW]
```

## How Sanctifier detects it

The rule uses a `syn::visit::Visit` pass over each `#[contractimpl]`. For every
public function whose name matches the getter/view naming heuristics, it flags a
storage `set` / `update` / `remove` call in the body. TTL-extension calls are
excluded, and `#[cfg(test)]` modules are skipped.

**Limitations:** the trigger is name-based, so a state-mutating function that
*isn't* named like a getter is out of scope (that is `sanct_visibility`'s job),
and a getter that writes through an aliased helper the rule can't see is a false
negative. Conversely, a deliberately memoising getter is a false positive — rename
or suppress it.

## References

- Soroban docs — [Persisting Data](https://soroban.stellar.org/docs/fundamentals-and-concepts/persisting-data)
- [CWE-1265: Unintended Reentrant Invocation of Non-reentrant Code](https://cwe.mitre.org/data/definitions/1265.html)
- Related: [`sanct_visibility`](sanct_visibility.md), [`view_panic`](view_panic.md)
