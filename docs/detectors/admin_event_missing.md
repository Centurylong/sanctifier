# `admin_event_missing` — Admin/config function mutates storage without emitting an event

| | |
| --- | --- |
| **Finding code** | [`SANCT_ADMIN_EVENT_MISSING`](../error-codes.md) |
| **Category** | events |
| **Severity** | Warning |
| **Source rule** | [`rules/admin_event_missing.rs`](../../tooling/sanctifier-core/src/rules/admin_event_missing.rs) |
| **Glossary** | [Event](../glossary.md#event) · [Persistent storage](../glossary.md#persistent-storage) |

## What it catches

A `#[contractimpl]` function whose name indicates an admin or configuration-change intent
(e.g. `set_`, `update_`, `change_`, `upgrade`, `pause`, `unpause`, `migrate`, `set_admin`,
`set_owner`, `configure`, `transfer_admin`) that performs a storage mutation —
`.set(…)`, `.update(…)`, or `.remove(…)` — **without** emitting a corresponding on-chain event.

Off-chain monitors, indexers, dashboards, and governance tools track privileged changes via
events. A silent storage write makes those changes invisible, preventing real-time alerting
and complicating incident response.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    pub fn set_admin(env: Env, new_admin: Address) {
        // Writes storage but never emits an event — monitors are blind.
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }
}
```

## The fix

Emit an event after every admin-level storage mutation:

```rust
#[contractimpl]
impl Token {
    pub fn set_admin(env: Env, new_admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events().publish(
            (symbol_short!("admin"), symbol_short!("set_adm")),
            new_admin,
        );
    }
}
```

If an event emit is genuinely unnecessary for a specific function, suppress the finding:

```rust
// sanctifier:ignore[SANCT_ADMIN_EVENT_MISSING]
pub fn migrate(env: Env) {
    env.storage().instance().set(&DataKey::Version, &2u32);
}
```

## How Sanctifier detects it

The rule uses a `syn::visit::Visit` pass. For every public function in a `#[contractimpl]`
block whose name matches the admin/config heuristic, it runs a body visitor that sets two
flags: `has_mutation` (`.set`/`.update`/`.remove` on a storage receiver) and `has_event`
(`.events()` in any receiver chain, or a call to `publish`/`emit`/`log`). A violation is
emitted only when `has_mutation && !has_event`.

`#[cfg(test)]` modules and functions annotated with
`// sanctifier:ignore[SANCT_ADMIN_EVENT_MISSING]` are skipped.

**Limitations:** detection is name-based, so an admin function with an atypical name is a
false negative. A function that delegates its event emit to an opaque helper may also be
missed. Conversely, a function that genuinely does not need an event (e.g. an internal
migration guard) can be suppressed.

## References

- Soroban docs — [Events](https://soroban.stellar.org/docs/fundamentals-and-concepts/events)
- Related: [`auth_gap`](auth_gap.md), [`state_write_in_view`](state_write_in_view.md)
