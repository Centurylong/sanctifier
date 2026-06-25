# Canonical vulnerable-contract gallery

Ten minimal Soroban contracts, each isolating **exactly one** bug class, paired
with a fixed counterpart. They are the shared corpus that backs the detector
golden-snapshot suite (`tests/gallery_snapshots.rs`) and double as best-practices
teaching material.

Each `*_vulnerable.rs` contains a single, clearly-commented flaw; each
`*_fixed.rs` is the same contract with the minimal correct fix. The fixtures are
parsed by the detectors with `syn`, so they only need to be valid Rust — they
are not compiled or deployed.

## Bug class → finding code

| # | Bug class | Finding code | Vulnerable fixture | Fixed fixture | Detector coverage |
|---|-----------|--------------|--------------------|---------------|-------------------|
| 1 | Re-initialization | `S001` auth_gap | `reinit_vulnerable.rs` | `reinit_fixed.rs` | ✅ flagged today |
| 2 | Unchecked upgrade auth | `S010` upgrade_risk | `upgrade_auth_vulnerable.rs` | `upgrade_auth_fixed.rs` | ⚠️ surfaced via `S001` auth_gap |
| 3 | CEI / reentrancy | `S006` unsafe_pattern | `reentrancy_vulnerable.rs` | `reentrancy_fixed.rs` | 🔜 planned detector |
| 4 | Unbounded loop | `S006` unsafe_pattern | `unbounded_loop_vulnerable.rs` | `unbounded_loop_fixed.rs` | 🔜 planned detector |
| 5 | Missing TTL bump | `S006` unsafe_pattern | `missing_ttl_vulnerable.rs` | `missing_ttl_fixed.rs` | ✅ default detector |
| 6 | Weak randomness | `S006` unsafe_pattern | `weak_randomness_vulnerable.rs` | `weak_randomness_fixed.rs` | 🔜 planned detector |
| 7 | Integer overflow | `S003` arithmetic_overflow | `integer_overflow_vulnerable.rs` | `integer_overflow_fixed.rs` | ✅ flagged today |
| 8 | Allowance race (TOCTOU) | `S006` unsafe_pattern | `allowance_race_vulnerable.rs` | `allowance_race_fixed.rs` | 🔜 planned detector |
| 9 | Oracle staleness | `S006` unsafe_pattern | `oracle_staleness_vulnerable.rs` | `oracle_staleness_fixed.rs` | 🔜 planned detector |
| 10 | Confused-deputy auth | `S001` auth_gap (family) | `confused_deputy_vulnerable.rs` | `confused_deputy_fixed.rs` | 🔜 planned refinement |

Finding codes are defined in `src/finding_codes.rs` and documented in
[`docs/error-codes.md`](../../../../../docs/error-codes.md).

### Coverage legend

- **✅ flagged today** — a default detector reports the bug on the vulnerable
  fixture and stays silent on the fixed one. You can see this in the committed
  snapshots (`tests/snapshots/gallery_snapshots__*.snap`).
- **⚠️ surfaced via …** — no dedicated detector yet, but an existing detector
  already catches the underlying mistake (e.g. the unauthenticated mutation).
- **🔜 planned detector** — no detector covers this class yet. The vulnerable
  fixture currently produces an **empty** snapshot. That empty snapshot is the
  regression baseline: when the dedicated `[DETECTOR]` lands, its snapshot will
  change and must be reviewed, proving the new detector fires on the canonical
  fixture.

## Adding to the gallery

1. Add `fixtures/gallery/<bug>_vulnerable.rs` and `<bug>_fixed.rs`.
2. Add a `gallery_case!` pair in `tests/gallery_snapshots.rs`.
3. Add a row to the table above with the mapped finding code.
4. `cargo insta test -p sanctifier-core` then `cargo insta review`, and commit
   the generated `.snap` files.
