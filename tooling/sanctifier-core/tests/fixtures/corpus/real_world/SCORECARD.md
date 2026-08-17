# Real-World Corpus Scorecard

Recomputed by running `RuleRegistry::with_default_rules().run_all(...)`
(the same detector suite `sanctifier analyze` uses) against every contract
in this corpus. See `real_world_corpus_test.rs` for the reproducible
harness and `MANIFEST.json` for contract provenance/labels.

**Corpus size:** 8 contracts (incremental toward the 20+ target — see
`MANIFEST.json`'s `status` field).

## Findings per contract

| Contract          | Findings |
|--------------------|---------:|
| `atomic_swap.rs`   |        5 |
| `increment.rs`     |        1 |
| `liquidity_pool.rs`|       65 |
| `pause.rs`         |        4 |
| `single_offer.rs`  |       17 |
| `timelock.rs`      |       14 |
| `ttl.rs`           |        3 |
| `account.rs`       |       12 |
| **Total**          |  **121** |

## Findings per detector

| Detector                | Count |
|--------------------------|------:|
| `unhandled_result`        |    43 |
| `SANCT_TTL_MISSING`       |    30 |
| `panic_detection`         |    20 |
| `arithmetic_overflow`     |    14 |
| `division_by_zero`        |     6 |
| `auth_gap`                |     3 |
| `SANCT_UNWRAP`            |     2 |
| `SANCT_VISIBILITY`        |     2 |
| `SANCT_ARG_DOS`           |     1 |

## Reading these numbers

These are **not** all false positives — several are legitimate,
Sanctifier catching real patterns even in official reference code (e.g.
`unhandled_result`/`panic_detection` firing on `.unwrap()` calls that these
example contracts use for brevity, which is exactly the pattern the
detector exists to flag in production code). The purpose of this corpus is
not "these contracts should score zero findings" — it's to give a stable,
real-code baseline: a detector change that suddenly doubles or triples this
total without a corresponding code change is a signal worth investigating,
which is what `real_world_corpus_finding_volume_is_bounded` guards against.

## Recomputing

```bash
cargo test -p sanctifier-core --test real_world_corpus_test
```

Re-run and update this file whenever the corpus grows (target: 20+
contracts) or a detector's behavior changes materially.
