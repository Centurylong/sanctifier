# Differential testing vs Slither / Aderyn

> Tracking issue: **#503** — feeds the **[HACKATHON] Competitive analysis (#166)**.

This document records a differential study of Sanctifier against the two most
widely used open-source smart-contract static analyzers, **[Slither]** and
**[Aderyn]**, on the checks that overlap. It quantifies where Sanctifier already
matches established tooling, where it has gaps worth closing, and where its
target platform makes a check unique or unnecessary.

[Slither]: https://github.com/crytic/slither
[Aderyn]: https://github.com/Cyfrin/aderyn

## TL;DR

On the ten canonical bug classes in the shared corpus:

| Analyzer | Classes flagged by default | Notes |
|----------|:--------------------------:|-------|
| **Sanctifier** | **3 / 10** | `reinit` (S001), `upgrade_auth` (S001), `integer_overflow` (S003) |
| **Slither** | 5 / 10 | reentrancy, unbounded loop, weak PRNG, confused deputy, unprotected upgrade |
| **Aderyn** | 6 / 10 | the Slither set + unprotected initializer (`reinit`) |

Sanctifier has **four detectable gaps** — classes an EVM tool already catches but
Sanctifier does not: **reentrancy, unbounded loop, weak randomness, and confused
deputy**. It is also the **only** analyzer that covers the Soroban-specific
**missing-TTL** class, and it is intentionally **stricter on integer overflow**
than the EVM tools (see [Divergences](#divergences)).

## Why a class-level comparison

Sanctifier analyzes **Stellar Soroban** contracts (Rust → Wasm); Slither and
Aderyn analyze **Solidity/EVM**. There is no common source language, so a
line-for-line cross-run is impossible. The issue scopes this correctly —
*"where checks overlap"* — so the comparison is made at the **vulnerability-class**
level: for each bug class, does each tool ship a default detector that catches it?

To keep the EVM side reproducible (not just a literature claim), the corpus also
ships **minimal Solidity mirrors** of the classes that have a direct EVM
analogue, so Slither and Aderyn can be run live by anyone who has them installed.

## The shared corpus

```
tooling/sanctifier-core/tests/fixtures/
├── gallery/                         # Soroban side (reused from issue #388)
│   ├── <bug>_vulnerable.rs
│   └── <bug>_fixed.rs
└── corpus/
    ├── differential-corpus.json     # manifest: class → fixtures → codes → detectors → overlap
    └── solidity/                    # EVM mirrors for the overlapping classes
        ├── <bug>_vulnerable.sol
        └── <bug>_fixed.sol
```

The **manifest** (`differential-corpus.json`) is the single source of truth. For
each class it records the Soroban fixtures, the finding codes Sanctifier emits
*today* (`sanctifier.observed_codes`), the closest default Slither/Aderyn
detectors, and an `overlap` classification.

## Running the harness

```bash
# Sanctifier side only (runs in CI, prints the matrix):
cargo test -p sanctifier-core --test differential_test -- --nocapture
#   …without a local Z3 install, add --no-default-features

# Full harness (also runs Slither/Aderyn over the .sol mirrors when installed):
./scripts/differential-test.sh
```

The Rust harness (`tests/differential_test.rs`) runs the default `RuleRegistry`
over every corpus fixture and **asserts** that the recorded ground truth still
holds — so the corpus can never silently drift from real detector behaviour.
It also fails if a `*_fixed` fixture produces any finding (false-positive guard).

## Overlap matrix

Legend: ✅ flagged by default · ⚠️ surfaced via a related detector · 🔜 planned,
not yet implemented · — no equivalent / not flagged.

| # | Bug class | Sanctifier | Slither detector(s) | Aderyn detector(s) | Overlap |
|---|-----------|:----------:|---------------------|--------------------|---------|
| 1 | Re-initialization | ✅ `S001` | — | `unprotected-initializer` | shared-covered |
| 2 | Unchecked upgrade auth | ⚠️ `S001` (`S010` planned) | `unprotected-upgrade` | `centralization-risk` | shared-covered |
| 3 | CEI / reentrancy | 🔜 `S006` | `reentrancy-eth`, `reentrancy-no-eth` | `state-change-after-external-call` | **sanctifier-gap** |
| 4 | Unbounded loop / DoS | 🔜 `S006` | `calls-loop`, `costly-loop` | `costly-operations-inside-loops` | **sanctifier-gap** |
| 5 | Missing TTL bump | 🔜 `S006` | — | — | **soroban-specific** |
| 6 | Weak randomness | 🔜 `S006` | `weak-prng` | `weak-randomness` | **sanctifier-gap** |
| 7 | Integer overflow | ✅ `S003` | — | — | **divergent-approach** |
| 8 | Allowance race (TOCTOU) | 🔜 `S006` | — | — | mutual-gap |
| 9 | Oracle staleness | 🔜 `S006` | — | — | mutual-gap |
| 10 | Confused-deputy auth | 🔜 `S001` family | `tx-origin`, `arbitrary-send-eth` | `arbitrary-from-in-transfer-from` | **sanctifier-gap** |

## Divergences

**Integer overflow (#7) — Sanctifier is stricter on purpose.** Sanctifier flags
unchecked `+`/`-`/`*` on Rust integers because the Soroban toolchain wraps on
overflow in release builds. On Solidity ≥ 0.8 the *compiler* inserts overflow
checks, so Slither and Aderyn correctly do **not** flag plain arithmetic — the
risk only returns inside an explicit `unchecked { }` block, which their default
rulesets also leave alone. Same class, opposite default, and both are right for
their platform. This is the clearest example of why platform context matters.

**Re-init / upgrade auth (#1, #2) — same outcome, different code.** Sanctifier
catches both via the presence-based `auth_gap` (S001) detector: the underlying
mistake is a state-mutating admin entrypoint with no `require_auth`. Aderyn has
a dedicated `unprotected-initializer`; Slither has `unprotected-upgrade`.
Functionally aligned; Sanctifier should still split out a dedicated `S010`
upgrade detector so the *report* names the right class (currently surfaced as
S001).

**Confused deputy (#10) — a false negative we can see.** The vulnerable fixture
*does* call `require_auth`, just on the wrong (attacker-influenceable) party, so
the presence-only `auth_gap` check stays silent and Sanctifier reports nothing.
Aderyn’s `arbitrary-from-in-transfer-from` and Slither’s `tx-origin` catch the
EVM analogue. This is a genuine coverage gap, not a platform difference.

**Mutual gaps (#8, #9).** The ERC-20 approve race and oracle-staleness classes
are real on both platforms but are not in *any* of the three default rulesets
(EVM tooling usually handles them with project-specific semgrep/custom rules).
Worth flagging as an industry-wide gap, not a Sanctifier-specific one.

## Action items / follow-up issues

Recommended issues to file from this study (priority order):

1. **`[DETECTOR] Reentrancy / CEI (S006)`** — highest-value gap; both EVM tools
   ship it. Detect external/host interaction before a storage effect.
2. **`[DETECTOR] Weak randomness (S006)`** — flag randomness derived from ledger
   sequence/timestamp; mirrors Slither `weak-prng` / Aderyn `weak-randomness`.
3. **`[DETECTOR] Unbounded loop over caller-controlled data (S006)`** — mirrors
   Slither `calls-loop`; pairs naturally with the existing gas estimator.
4. **`[REFINE] Confused-deputy / arbitrary-from for auth_gap (S001)`** — move
   `auth_gap` from "is `require_auth` present" to "is the *right principal*
   authorized", closing the #10 false negative.
5. **`[REPORT] Dedicated S010 upgrade-risk finding`** — stop surfacing upgrade
   auth as S001 so reports name the class correctly.
6. **(stretch) `[DETECTOR] Missing-TTL (S006)`** — Soroban-specific; no EVM
   tool can help here, so it is uniquely Sanctifier’s to own.

## Limitations & methodology notes

- **Class-level, not line-level.** Because the source languages differ, equality
  means "both ship a default detector for this class", not identical findings.
- **Approximate cross-language fixtures.** The Solidity mirrors reproduce the
  *pattern* of each bug, not the exact Soroban code. They are written for
  `solc ^0.8.20`; the live Slither/Aderyn rows are reproduced by
  `scripts/differential-test.sh` when those tools are installed. Where they are
  not, the Slither/Aderyn columns are sourced from each tool's published
  detector catalog and clearly marked as documentation in the harness output.
- **Default rulesets only.** Custom/optional detectors and plugins are out of
  scope; the comparison is "out of the box" coverage.
- **Ground truth is enforced.** `sanctifier.observed_codes` is asserted against
  live detector output by `tests/differential_test.rs`, and is kept in lock-step
  with the committed gallery snapshots (`tests/snapshots/gallery_snapshots__*`).
