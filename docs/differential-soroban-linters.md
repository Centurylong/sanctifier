# Differential testing vs other Soroban linters

> Tracking issue: **#763** — Testing Infrastructure epic. Companion to the
> EVM-oriented [Differential testing vs Slither/Aderyn](differential-testing.md)
> (issue #503).

The Slither/Aderyn study compares Sanctifier to the established **EVM/Solidity**
analyzers at the vulnerability-class level, because those tools target a
different platform. This document does the complementary study against tools that
target **the same platform Sanctifier does — Stellar Soroban (Rust → Wasm)** — so
here we can compare **detector-for-detector**, not just class-for-class.

The goal, per the issue, is to *"compare findings against any other available
Soroban analyzers to find gaps/disagreements"* and to *"reveal blind spots and
false positives"*. Where the tools agree, that is mutual validation. Where they
disagree, one of them has a gap worth closing.

## The Soroban analyzer landscape

Unlike EVM, the Soroban static-analysis ecosystem is young. The tools that
actually inspect Soroban Rust source today:

| Tool | Kind | Soroban-aware? | Role in this study |
|------|------|:--------------:|--------------------|
| **[CoinFabrik Scout]** (`cargo-scout-audit`) | dedicated security linter for ink!/Soroban | **yes** | primary comparison target |
| **`cargo clippy`** | general Rust linter | no (Rust-generic) | baseline: catches Rust mistakes, not Soroban security classes |
| **Semgrep** (custom rules) | pattern matcher | only via hand-written rules | out of the box: nothing Soroban-specific |
| **`cargo-geiger`** | `unsafe` usage counter | no | orthogonal (unsafe surface, not vuln classes) |

[CoinFabrik Scout]: https://github.com/CoinFabrik/scout-audit

**Scout is the only other tool that ships default, Soroban-specific security
detectors**, so it is the substantive comparison here. `clippy` is included as
the "free baseline everyone already runs" and, importantly, it overlaps almost
not at all with Sanctifier — the two are complementary, not redundant.

## Methodology

- Comparison is at the **detector level**: for each Scout detector, is there a
  Sanctifier finding code that catches the same defect, and vice-versa?
- The Scout column is sourced from Scout's **published detector catalog** and is
  marked as documentation, not a live cross-run. A reproducible live harness that
  runs `cargo-scout-audit` over the shared gallery fixtures is filed as a
  follow-up below (it requires the Scout toolchain in CI, which is heavier than
  the pure-Rust harness we already run for the EVM study).
- Sanctifier codes are ground truth: they are the codes the default
  `RuleRegistry` emits today, enforced by `tests/differential_test.rs` and the
  committed gallery snapshots. See [Finding Codes](error-codes.md).

## Detector overlap matrix

Legend: ✅ both flag it · ⚠️ partial / surfaced via a broader detector ·
🔜 planned · — not covered.

| Defect class | Scout detector | Sanctifier | Status |
|--------------|----------------|:----------:|--------|
| `unwrap`/`expect` panics | `unsafe-unwrap`, `unsafe-expect` | ✅ `SANCT_UNWRAP`, `S002` | **agree** |
| Explicit panic / assert | `avoid-panic-error`, `assert-violation` | ✅ `S002` | **agree** |
| Unchecked overflow | `overflow-check` | ✅ `S003` | **agree** |
| Divide-before-multiply | `divide-before-multiply` | ⚠️ partial (`S003` is order-agnostic) | Sanctifier gap |
| Incorrect exponentiation | `incorrect-exponentiation` | — | Sanctifier gap |
| Weak randomness | `insufficiently-random-values` | 🔜 (also a gap vs EVM tools) | Sanctifier gap |
| Unbounded loop / DoS | `dos-unbounded-operation` | ✅ `SANCT_ARG_DOS` | **agree** |
| DoS via unexpected revert | `dos-unexpected-revert-with-vector` | ⚠️ partial (`SANCT_ARG_DOS` neighbours) | Sanctifier gap |
| Unprotected upgrade | `unprotected-update-current-contract-wasm` | ⚠️ surfaced via `S001` | **agree (partial)** |
| Unprotected storage write | `set-contract-storage` | ⚠️ surfaced via `S001` auth_gap | **agree (partial)** |
| Unprotected mapping op | `unprotected-mapping-operation` | ⚠️ surfaced via `S001` | **agree (partial)** |
| Ignored return enum | `unused-return-enum` | ⚠️ neighbours `S009` unhandled_result | **agree (partial)** |
| Zero / test address | `zero-or-test-address` | ⚠️ neighbours `S012` hardcoded_addr | **agree (partial)** |
| SDK version hygiene | `soroban-version` | — | Scout-only |
| `core::mem::forget` misuse | `avoid-core-mem-forget` | — | Scout-only |
| Indexing vs iterators | `iterators-over-indexing` | — | Scout-only (style/safety) |
| **Missing storage TTL** | — | ✅ `S006` (`SANCT_TTL_MISSING`) | **Sanctifier-only** |
| **State write in a view/getter** | — | ✅ `SANCT_STATE_WRITE_IN_VIEW` | **Sanctifier-only** |
| Fee/rounding drift | — | ✅ `S017` | **Sanctifier-only** |
| Error-code collision | — | ✅ `S016` | **Sanctifier-only** |
| Ledger-size / OOG estimation | — | ✅ `S004` | **Sanctifier-only** |

## What the comparison reveals

### Agreements (mutual validation)
On the classes both tools ship — **panics/`unwrap`, explicit panic/assert,
unchecked overflow, and unbounded-loop DoS** — Sanctifier and Scout independently
flag the same defect. Two tools built by different teams converging on the same
findings is the strongest available signal that these detectors are sound and the
classes matter.

### Sanctifier gaps (Scout catches, we don't — yet)
1. **Weak / predictable randomness** (`insufficiently-random-values`). This is a
   gap against *both* Scout and the EVM tools (Slither `weak-prng`, Aderyn
   `weak-randomness`) — the clearest, highest-priority hole in Sanctifier's
   coverage.
2. **Divide-before-multiply** precision loss. Sanctifier's `S003` flags unchecked
   arithmetic but is agnostic to operation *ordering*; Scout specifically catches
   the precision-losing order.
3. **Incorrect exponentiation** (`^` used where `pow` was meant — a classic Rust
   footgun that also bites Soroban authors). No Sanctifier equivalent.

### Scout gaps (we catch, Scout doesn't)
Sanctifier is the **only** tool that flags several genuinely Soroban-native
classes: **missing storage TTL** (state archival — unique to Soroban's ledger
model), **state writes inside view/getter functions**, **fee/rounding drift**,
**error-code collisions**, and **ledger-size/OOG estimation**. These are exactly
the areas where Sanctifier's Soroban specialisation pays off over a more general
Rust-security linter.

### Disagreements / false-positive risk
The `⚠️ partial` rows are where the tools *nominally* overlap but at different
granularity, which is where false positives and false negatives hide:

- **Auth family** (`unprotected-upgrade`, `set-contract-storage`,
  `unprotected-mapping-operation`). Scout ships *dedicated* detectors per
  operation; Sanctifier surfaces all of them through one broad presence-based
  `S001 auth_gap`. Sanctifier's single detector is more prone to **false
  negatives** (it only asks "is `require_auth` present", not "is the *right*
  principal authorized" — the same confused-deputy blind spot documented in the
  [EVM study](differential-testing.md#divergences)). Scout's finer detectors are
  more prone to **false positives** on intentionally-public operations. Neither is
  strictly better; the split motivates refining `S001` (below).
- **Return/enum handling** (`unused-return-enum` vs `S009`) and **zero-address**
  (`zero-or-test-address` vs `S012`) overlap in intent but not in exact trigger,
  so their outputs will not line up one-to-one on real contracts.

## Action items / follow-up issues

Recommended issues to file from this study (priority order):

1. **`[DETECTOR] Weak randomness`** — highest-value gap; flagged by Scout *and*
   both EVM tools. Detect randomness derived from ledger sequence/timestamp.
   (Consolidates with action item #2 in the [EVM study](differential-testing.md#action-items--follow-up-issues).)
2. **`[DETECTOR] Divide-before-multiply precision loss`** — order-sensitive
   arithmetic check to complement the order-agnostic `S003`.
3. **`[DETECTOR] Incorrect exponentiation (^ vs pow)`** — cheap, high-signal.
4. **`[REFINE] Split the S001 auth family`** — decompose the broad `auth_gap`
   into upgrade / storage-write / mapping-op variants so reports name the class
   Scout-precisely and the confused-deputy false negative can be closed.
5. **`[HARNESS] Live cargo-scout-audit cross-run`** — extend
   `scripts/differential-test.sh` to run Scout over the gallery fixtures when the
   Scout toolchain is present, so this matrix becomes machine-verified like the
   Sanctifier side already is.
6. **(consider) `[DETECTOR] Soroban SDK version hygiene`** — mirror Scout's
   `soroban-version`; low security value but easy adoption signal.

## Limitations & methodology notes

- **Catalog-sourced Scout column.** Until action item #5 lands, the Scout rows
  are read from Scout's published detector list, not a live run — so this matrix
  documents *intended* coverage, exactly as the EVM study does for the
  documentation-only Slither/Aderyn rows.
- **Default rulesets only.** Custom Scout detectors, Semgrep rule packs, and
  optional plugins are out of scope; the comparison is out-of-the-box coverage.
- **`clippy` is deliberately excluded from the matrix.** It flags general Rust
  issues (dead code, needless clones, `clippy::correctness`) but *no* Soroban
  security class, so it neither agrees nor disagrees with Sanctifier — run both.
- **Overlap ≠ identical output.** A shared class means both tools *aim* at the
  defect; on a given contract their exact findings, locations, and false-positive
  profiles will still differ, which is precisely what the live harness (#5) will
  quantify.

## See also

- [Differential testing vs Slither/Aderyn](differential-testing.md) — the EVM
  counterpart study.
- [Positioning](positioning.md) — where Sanctifier fits vs audits and tooling.
- [Finding Codes](error-codes.md) — the codes referenced above.
- [Awesome Soroban Security](awesome-soroban-security.md) — external tools and
  resources.
