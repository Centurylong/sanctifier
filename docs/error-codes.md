# Sanctifier Error Code Mapping

Sanctifier now uses a unified finding code system across `sanctifier-core` and `sanctifier-cli` outputs.

| Code | Category | Meaning |
|------|----------|---------|
| `S001` | authentication | Missing authentication guard in a state-mutating function |
| `S002` | panic_handling | `panic!` / `unwrap` / `expect` usage that may abort execution |
| `S003` | arithmetic | Unchecked arithmetic with overflow/underflow risk |
| `S004` | storage_limits | Ledger entry size exceeds or approaches configured limits |
| `S005` | storage_keys | Potential storage key collision |
| `S006` | unsafe_patterns | Potentially unsafe language/runtime pattern |
| `S007` | custom_rule | User-defined custom rule match |
| `SANCT_ADDRESS_VALIDATION` | input_validation | Sensitive `Address` parameter is stored or used without explicit invalid-address validation |
| `SANCT_UNWRAP` | panic_handling | `unwrap` / `expect` / risky `unwrap_or_default` inside `#[contractimpl]` entrypoints; replace with typed errors or explicit domain defaults |

## Detector catalog

### `SANCT_ADDRESS_VALIDATION`

Flags Soroban `#[contractimpl]` entrypoints such as admin, owner, asset, token,
recipient, or transfer setters that accept an `Address` and then store or use it
without an explicit invalid-address or zero-address guard.

```rust
#[contractimpl]
impl Config {
    pub fn set_admin(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }
}
```

Validate sensitive addresses before persisting them or moving value. The detector
recognizes local helpers such as `validate_address(&admin)`, `ensure_valid_address`,
`reject_zero_address`, and direct `is_zero` / `Address::zero` guards. Use
`sanctifier:ignore[SANCT_ADDRESS_VALIDATION]` on the line before an intentional
exception. `require_auth` alone is not treated as address validation because it
proves authorization, not that the supplied address is a safe configuration
target.

### `SANCT_UNWRAP`

Flags `unwrap()`, `expect(..)`, and risky `unwrap_or_default()` calls inside
Soroban `#[contractimpl]` entrypoints. In an entrypoint, an attacker-triggered
missing value can abort the whole transaction or silently turn absent financial
state into a default value.

```rust
#[contractimpl]
impl Token {
    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage().persistent().get(&DataKey::Balance(id)).unwrap_or_default()
    }
}
```

Prefer explicit handling: return a typed `Result`, map missing state to a
domain-specific `Error`, or use an explicit default such as `unwrap_or(0)` only
when zero is the intended contract state.

## Where codes appear

- Text output from `sanctifier analyze`
- JSON report output under:
  - `error_codes` (full mapping table)
  - each item inside `findings.*` as `code`
