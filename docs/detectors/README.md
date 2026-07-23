<!--
  Per-detector documentation index.

  Every detector registered in RuleRegistry::with_default_rules()
  (tooling/sanctifier-core/src/rules/mod.rs) MUST have a page in this directory
  named `<detector_name>.md`, and MUST appear in the table below. This is
  enforced in CI by tests/detector_docs_coverage.rs — if you add a detector,
  add its page here or the build fails.
-->

# Detector Catalog

One page per Sanctifier detector: **what it catches**, a **vulnerable example**,
**the fix**, and **references**. Together these pages double as a security
curriculum for Stellar [Soroban](https://soroban.stellar.org/) contract authors.

Each finding Sanctifier emits carries a stable code (see
[Finding Codes](../error-codes.md)); the table below links every code to its
detector page and to the relevant [Glossary](../glossary.md) term.

| Detector | Code | Category | Severity | Catches |
| --- | --- | --- | --- | --- |
| [`auth_gap`](auth_gap.md) | [`S001`](../error-codes.md) | authentication | Critical | State-mutating entrypoints missing `require_auth` |
| [`panic_detection`](panic_detection.md) | [`S002`](../error-codes.md) | panic_handling | High | `panic!` / `unwrap` / `expect` that trap the invocation |
| [`arithmetic_overflow`](arithmetic_overflow.md) | [`S003`](../error-codes.md) | arithmetic | High | Unchecked `+` `-` `*` that can overflow or underflow |
| [`ledger_size`](ledger_size.md) | [`S004`](../error-codes.md) | storage_limits | Medium | `contracttype` layouts approaching the ledger entry size limit |
| [`missing_ttl`](missing_ttl.md) | [`S006`](../error-codes.md) | storage_durability | Medium | Persistent/instance storage access without a TTL extension |
| [`unhandled_result`](unhandled_result.md) | [`S009`](../error-codes.md) | logic | Medium | A `Result` that is silently dropped |
| [`hardcoded_addr`](hardcoded_addr.md) | [`S012`](../error-codes.md) | code_hygiene | High | Hardcoded admin address / secret literal in an auth context |
| [`edge_amount`](edge_amount.md) | [`S013`](../error-codes.md) | code_hygiene | Medium | `transfer`/`mint`/`burn` missing `amount > 0` / `from != to` guards |
| [`unused_variable`](unused_variable.md) | [`S015`](../error-codes.md) | code_hygiene | Info | Unused local bindings (dead code) |
| [`error_code_collision`](error_code_collision.md) | [`S016`](../error-codes.md) | code_hygiene | Medium | Duplicate/inconsistent `#[contracterror]` discriminants |
| [`fee_rounding`](fee_rounding.md) | [`S017`](../error-codes.md) | arithmetic | High | Integer-division fees that round to zero for micro-amounts |
| [`unsigned_underflow`](unsigned_underflow.md) | [`S019`](../error-codes.md) | arithmetic | High | Unchecked `-` / `-=` on an unsigned integer that wraps past zero |
| [`arg_dos`](arg_dos.md) | [`SANCT_ARG_DOS`](../error-codes.md) | denial_of_service | High | `Vec`/`Map` arguments iterated without a length cap |
| [`sanct_unwrap`](sanct_unwrap.md) | [`SANCT_UNWRAP`](../error-codes.md) | panic_handling | High | `unwrap`/`expect`/risky default in `#[contractimpl]` entrypoints |
| [`sanct_visibility`](sanct_visibility.md) | [`SANCT_VISIBILITY`](../error-codes.md) | authentication | High | Helper-shaped state mutator exported without an auth guard |
| [`unbounded_storage`](unbounded_storage.md) | [`SANCT_UNBOUNDED_STORAGE`](../error-codes.md) | denial_of_service | High | Persistent/instance collection grows with no removal or length cap |
| [`view_panic`](view_panic.md) | [`SANCT_VIEW_PANIC`](../error-codes.md) | panic_handling | Medium | View/getter entrypoint contains a reachable panic |
| [`allowance_race`](allowance_race.md) | [`SANCT_ALLOWANCE_RACE`](../error-codes.md) | authorization | Medium | `approve` overwrites the allowance unconditionally (approve TOCTOU) |
| [`state_write_in_view`](state_write_in_view.md) | [`SANCT_STATE_WRITE_IN_VIEW`](../error-codes.md) | code_hygiene | Warning | Getter/view-named function performs a storage write |
| [`division_by_zero`](division_by_zero.md) | [`S018`](../error-codes.md) | arithmetic | Medium | `/` or `%` by a non-constant value not proven non-zero |

## Page anatomy

Every detector page follows the same structure so they read as one body of work:

1. **Summary** — the code, category, severity, and the source rule.
2. **What it catches** — the vulnerability class in one paragraph.
3. **Vulnerable example** — a minimal Soroban contract that trips the detector.
4. **The fix** — the same contract, corrected.
5. **How Sanctifier detects it** — the analysis technique, and its limits.
6. **References** — Soroban docs, CWE entries, and related detectors.

## See also

- [Finding Codes](../error-codes.md) — the code → meaning table.
- [Glossary](../glossary.md) — 50 Soroban/Stellar security terms.
- [Detector Cookbook](../detector-cookbook.md) — how to *write* a new detector.
- [Awesome Soroban Security](../awesome-soroban-security.md) — external resources.
