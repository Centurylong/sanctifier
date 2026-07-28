# Sanctifier Error Code Mapping

Sanctifier now uses a unified finding code system across `sanctifier-core` and `sanctifier-cli` outputs.

Each code emitted by a detector links to its full page in the
[Detector Catalog](detectors/README.md) — what it catches, a vulnerable example,
the fix, and references.

| Code | Category | Meaning | Detector page |
|------|----------|---------|---------------|
| `S001` | authentication | Missing authentication guard in a state-mutating function | [`auth_gap`](detectors/auth_gap.md) |
| `S002` | panic_handling | `panic!` / `unwrap` / `expect` usage that may abort execution | [`panic_detection`](detectors/panic_detection.md) |
| `S003` | arithmetic | Unchecked arithmetic with overflow/underflow risk | [`arithmetic_overflow`](detectors/arithmetic_overflow.md) |
| `S004` | storage_limits | Ledger entry size exceeds or approaches configured limits | [`ledger_size`](detectors/ledger_size.md) |
| `S005` | storage_keys | Potential storage key collision | — |
| `S006` | storage_durability | Persistent/instance storage access without a TTL extension | [`missing_ttl`](detectors/missing_ttl.md) |
| `S007` | custom_rule | User-defined custom rule match | — |
| `S009` | logic | A `Result` that is silently dropped | [`unhandled_result`](detectors/unhandled_result.md) |
| `S012` | code_hygiene | Hardcoded admin address / secret literal in an auth context | [`hardcoded_addr`](detectors/hardcoded_addr.md) |
| `S013` | code_hygiene | `transfer`/`mint`/`burn` missing `amount > 0` / `from != to` guards | [`edge_amount`](detectors/edge_amount.md) |
| `S015` | code_hygiene | Unused local binding (dead code) | [`unused_variable`](detectors/unused_variable.md) |
| `S016` | code_hygiene | Duplicate/inconsistent `#[contracterror]` discriminants | [`error_code_collision`](detectors/error_code_collision.md) |
| `S017` | arithmetic | Fee/interest integer division that rounds to zero for micro-amounts | [`fee_rounding`](detectors/fee_rounding.md) |
| `SANCT_ARG_DOS` | denial_of_service | `Vec`/`Map` argument iterated without a length cap | [`arg_dos`](detectors/arg_dos.md) |
| `SANCT_UNWRAP` | panic_handling | `unwrap` / `expect` / risky `unwrap_or_default` inside `#[contractimpl]` entrypoints; replace with typed errors or explicit domain defaults | [`sanct_unwrap`](detectors/sanct_unwrap.md) |
| `SANCT_VISIBILITY` | authentication | Helper-shaped state mutator exposed through `#[contractimpl]` without authorization | [`sanct_visibility`](detectors/sanct_visibility.md) |
| `SANCT_UNBOUNDED_STORAGE` | denial_of_service | Persistent/instance collection grows via append/insert with no removal or length cap | [`unbounded_storage`](detectors/unbounded_storage.md) |

> **Full catalog:** [Detector Catalog →](detectors/README.md)

### Source-optional (compiled WASM) codes

Emitted only by [`sanctifier wasm`](wasm-analysis.md), which analyzes a deployed
module directly. See [Source-Optional WASM Analysis](wasm-analysis.md) for the
full source-vs-WASM comparison.

| Code | Category | Meaning |
|------|----------|---------|
| `W001` | wasm | Compiled module has no Soroban contract spec section; may not be a Soroban contract |
| `W002` | wasm | Compiled module exports no callable functions |
| `W003` | wasm | Compiled module is missing Soroban environment metadata (interface version) |
| `W004` | wasm | Compiled module uses floating-point value types, which the Soroban host rejects |

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

### `SANCT_VISIBILITY`

Flags public helper-shaped methods inside `#[contractimpl]` that mutate contract
state without calling `require_auth()` or `require_auth_for_args()`. Leading
underscores and explicit `helper` or `internal` naming are treated as evidence
that a method was intended for internal use.

```rust
#[contractimpl]
impl Token {
    pub fn _set_balance(env: Env, owner: Address, amount: i128) {
        write_balance(&env, &owner, amount);
    }
}
```

Keep helpers private when possible. If a helper is intentionally exposed as a
contract entrypoint, authenticate the appropriate principal before any state
mutation.

## Where codes appear

- Text output from `sanctifier analyze`
- JSON report output under:
  - `error_codes` (full mapping table)
  - each item inside `findings.*` as `code`
