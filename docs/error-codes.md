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
| `SANCT_UNWRAP` | panic_handling | `unwrap` / `expect` / risky `unwrap_or_default` inside `#[contractimpl]` entrypoints; replace with typed errors or explicit domain defaults |
| `SANCT_SYMBOL_SHORT_LENGTH` | code_hygiene | `symbol_short!` literal exceeds Soroban's 9-byte limit; use `Symbol::new` for longer symbols |

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

### `SANCT_SYMBOL_SHORT_LENGTH`

Flags `symbol_short!("...")` calls whose string literal is longer than 9 UTF-8
bytes. `symbol_short!` is meant only for very short static symbols; longer
literals should be constructed with `Symbol::new(&env, "...")`.

```rust
const KEY: Symbol = symbol_short!("POSITION_ID");
```

## Where codes appear

- Text output from `sanctifier analyze`
- JSON report output under:
  - `error_codes` (full mapping table)
  - each item inside `findings.*` as `code`
