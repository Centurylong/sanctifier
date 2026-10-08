# Event emission completeness

## Summary
- **Finding code:** `SANCT_EVENT_EMISSION_GAP`
- **Category:** events
- **Severity:** Warning
- **Rule:** `event_emission_completeness`

Flags a public `#[contractimpl]` entrypoint that directly mutates Soroban storage without publishing an event when sibling entrypoints in the same contract implementation already establish an event surface.

## What it catches
Off-chain indexers can miss externally observable state transitions when a contract uses events for some state changes but silently mutates storage in another public entrypoint. The detector is deliberately conservative to keep false positives low.

## Vulnerable example
```rust
#[contractimpl]
impl Token {
    pub fn set_balance(env: Env, who: Address, amount: i128) {
        env.storage().persistent().set(&who, &amount);
    }

    pub fn set_balance_with_event(env: Env, who: Address, amount: i128) {
        env.storage().persistent().set(&who, &amount);
        env.events().publish((symbol_short!("balance"), who), amount);
    }
}
```

## The fix
Publish a compact event in the same externally callable entrypoint as the state mutation.

```rust
pub fn set_balance(env: Env, who: Address, amount: i128) {
    env.storage().persistent().set(&who, &amount);
    env.events().publish((symbol_short!("balance"), who), amount);
}
```

## Detection boundaries
The rule only considers public methods in `#[contractimpl]` blocks. It requires an existing event surface in the same impl, recognizes direct `set`, `update`, or `remove` calls on storage-like receivers, ignores TTL operations, and treats any same-entrypoint `env.events().publish(..)` call as satisfying the rule. It does not prove event payload semantics.

## References
- Soroban contract events
- `tooling/sanctifier-core/src/rules/event_emission_completeness.rs`
