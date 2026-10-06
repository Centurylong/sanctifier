# `sensitive_getter`

## Summary

| Field | Value |
| --- | --- |
| Finding code | `SANCT_SENSITIVE_GETTER` |
| Category | `information_exposure` |
| Severity | Info |
| Source rule | `sensitive_getter` |

Flags public Soroban getters that directly return storage entries whose names strongly indicate secrets, credentials, signing material, or similarly sensitive internal data.

## What it catches

A public `#[contractimpl]` entrypoint can expose data that was intended only for contract internals. This detector is deliberately conservative: it only reports value-returning public getter-like functions that directly call Soroban storage `.get(...)` and whose function name or storage-key expression contains a strong sensitive-data term such as `secret`, `credential`, `signing_key`, `mnemonic`, or `access_token`.

Ordinary metadata such as owner/admin addresses, versions, balances, and other non-sensitive state is not reported by this rule.

## Vulnerable example

```rust
#[contractimpl]
impl Vault {
    pub fn get_signing_key(env: Env) -> BytesN<32> {
        env.storage()
            .instance()
            .get(&DataKey::SigningKey)
            .unwrap()
    }
}
```

## The fix

Do not expose the secret value through a public contract entrypoint. Keep secrets or signing material off-chain when possible, or expose only non-sensitive derived metadata.

```rust
#[contractimpl]
impl Vault {
    pub fn get_signing_key_fingerprint(env: Env) -> BytesN<32> {
        env.storage()
            .instance()
            .get(&DataKey::SigningKeyFingerprint)
            .unwrap()
    }
}
```

## How Sanctifier detects it

The rule:

1. visits `#[contractimpl]` implementation blocks outside `#[cfg(test)]` modules;
2. considers public functions with a return value whose names look like getters (`get_`, `view_`, `read_`, `fetch_`, `query_`, or `peek_`) or are themselves sensitive-looking;
3. records direct `.get(...)` calls on persistent, temporary, or instance Soroban storage; and
4. emits `SANCT_SENSITIVE_GETTER` only when the getter name or storage-key expression contains a strong sensitive-data term.

This is intentionally not a dataflow engine. Helper-return aliases, values copied through intermediate helper functions, encrypted/derived semantics, and arbitrary custom key naming are out of scope for this focused advisory detector.

## References

- [Soroban storage](https://developers.stellar.org/docs/build/smart-contracts/data-storage)
- [CWE-200: Exposure of Sensitive Information to an Unauthorized Actor](https://cwe.mitre.org/data/definitions/200.html)
- [Detector Catalog](README.md)
