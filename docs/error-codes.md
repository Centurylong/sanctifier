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
| `SANCT_LEDGER_RANDOMNESS` | randomness | Ledger sequence or timestamp is used as predictable randomness |
| `SANCT_UNWRAP` | panic_handling | `unwrap` / `expect` / risky `unwrap_or_default` inside `#[contractimpl]` entrypoints; replace with typed errors or explicit domain defaults |

## Detector catalog

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

### `SANCT_LEDGER_RANDOMNESS`

Flags randomness-like code paths that derive a winner, seed, index, shuffle, or
nonce from `env.ledger().sequence()` or `env.ledger().timestamp()`. These values
are predictable and can be influenced by transaction timing.

Prefer commit-reveal, a VRF/oracle, or another user-independent entropy source
for randomness.

## Where codes appear

- Text output from `sanctifier analyze`
- JSON report output under:
  - `error_codes` (full mapping table)
  - each item inside `findings.*` as `code`
