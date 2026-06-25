# Differential-testing corpus (issue #503)

The shared corpus that backs the differential study of Sanctifier against
[Slither] and [Aderyn]. See the write-up in
[`docs/differential-testing.md`](../../../../../docs/differential-testing.md).

[Slither]: https://github.com/crytic/slither
[Aderyn]: https://github.com/Cyfrin/aderyn

## Layout

```
corpus/
├── differential-corpus.json   # manifest — single source of truth
└── solidity/                  # EVM mirrors of the overlapping classes
    ├── <bug>_vulnerable.sol
    └── <bug>_fixed.sol
```

The **Soroban** side of the corpus is *not* duplicated here — it reuses the
canonical gallery in [`../gallery/`](../gallery/README.md) (issue #388), one
vulnerable + fixed pair per bug class. `differential-corpus.json` references
those `.rs` files by name.

## `differential-corpus.json`

One entry per bug class:

| field | meaning |
|-------|---------|
| `soroban.{vulnerable,fixed}` | gallery fixtures analyzed by Sanctifier |
| `solidity.{vulnerable,fixed}` | EVM mirrors analyzed by Slither/Aderyn (`null` if the class has no EVM analogue) |
| `sanctifier.observed_codes` | finding codes the default `RuleRegistry` emits on the vulnerable fixture **today** (asserted by the harness) |
| `sanctifier.status` | `flagged` / `surfaced` / `planned` |
| `slither.detectors`, `aderyn.detectors` | closest default detectors each tool ships for the class |
| `{slither,aderyn}.expected` | whether that tool flags the class out of the box |
| `overlap` | one of the labels in `overlap_legend` |

## Harness

- **Sanctifier side** — `tests/differential_test.rs` runs every fixture and
  asserts `observed_codes` is still accurate (and that `*_fixed` fixtures stay
  clean). Kept in lock-step with the gallery snapshots.
- **EVM side** — `scripts/differential-test.sh` runs Slither/Aderyn over the
  `solidity/` mirrors when those tools are installed, and skips them gracefully
  otherwise.

## Adding a class

1. Add the Soroban pair under `../gallery/` (and wire its snapshot) if it is not
   already there.
2. If the class has an EVM analogue, add `solidity/<bug>_{vulnerable,fixed}.sol`.
3. Add an entry to `differential-corpus.json`; set `observed_codes` to match what
   Sanctifier actually emits (run the harness — it will fail until they agree).
4. Update the matrix in `docs/differential-testing.md`.
