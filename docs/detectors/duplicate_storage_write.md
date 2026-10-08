# Duplicate storage writes

## Summary

- **Finding code:** `SANCT_DUPLICATE_STORAGE_WRITE`
- **Category:** gas efficiency
- **Severity:** Warning
- **Rule:** `duplicate_storage_write`

Flags a repeated Soroban storage write when the same stable value is written to
the same storage kind and key twice in one straight-line statement sequence.
The second write is redundant and spends ledger/gas work without changing state.

## What it catches

```rust
fn save(env: Env, key: Symbol, value: i128) {
    env.storage().persistent().set(&key, &value);
    env.storage().persistent().set(&key, &value);
}
```

## The fix

Keep one write when the value is unchanged. Intentional updates remain valid:

```rust
fn save(env: Env, key: Symbol, first: i128, second: i128) {
    env.storage().persistent().set(&key, &first);
    env.storage().persistent().set(&key, &second);
}
```

## How Sanctifier detects it

The rule tracks direct `.set(key, value)` calls inside each straight-line block.
It reports only when storage kind, stable key expression, and stable value
expression all match the immediately retained write for that target. A
non-`set` statement clears the local equivalence state, and a different value
is treated as an intentional update. The rule does not attempt interprocedural
alias or data-flow analysis.

## References

- Soroban `Env::storage()` persistent, temporary, and instance storage APIs
- Issue #688: redundant / duplicate storage writes
