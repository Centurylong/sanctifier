# `storage_read_in_loop` — Loop-invariant storage read

| | |
| --- | --- |
| **Finding code** | [`SANCT_STORAGE_READ_IN_LOOP`](../error-codes.md) |
| **Category** | gas_efficiency |
| **Severity** | Warning |
| **Source rule** | [`rules/storage_read_in_loop.rs`](../../tooling/sanctifier-core/src/rules/storage_read_in_loop.rs) |

## What it catches

A Soroban storage `get` or `has` performed inside a loop when the storage key
is invariant across iterations. Repeating the same host storage read wastes gas
and budget when the value can be loaded once before the loop.

## Vulnerable example

```rust
pub fn scan(env: Env, users: Vec<Address>, config_key: DataKey) {
    for user in users.iter() {
        let fee: i128 = env.storage().persistent().get(&config_key).unwrap_or(0);
        charge(&user, fee);
    }
}
```

## The fix

Hoist the invariant read and reuse it:

```rust
pub fn scan(env: Env, users: Vec<Address>, config_key: DataKey) {
    let fee: i128 = env.storage().persistent().get(&config_key).unwrap_or(0);
    for user in users.iter() {
        charge(&user, fee);
    }
}
```

## How Sanctifier detects it

The rule walks `for`, `while`, and `loop` bodies, records loop variables and
variables assigned by the loop, and reports storage `get`/`has` calls whose
key expression does not reference any of those values. It deliberately
suppresses a loop when the loop mutates Soroban storage, because hoisting a read
across a write is not generally semantics-preserving without alias analysis.

Nested loops are analysed independently.

## Limitations

This is conservative syntax-level analysis. A helper that mutates storage
indirectly is not resolved interprocedurally, and algebraically invariant key
expressions that mention a mutated variable are intentionally not reported.

## References

- Stellar/Soroban resource metering and storage host operations
- Related: [`excessive_clone`](excessive_clone.md), [`cross_contract_call_in_loop`](cross_contract_call_in_loop.md)
