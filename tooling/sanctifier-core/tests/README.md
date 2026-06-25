# Detector golden snapshot tests

Every detector in `sanctifier-core` has a **golden snapshot** of its findings,
powered by [`insta`](https://insta.rs). This is the safety net that lets us add
and refactor detectors without silently regressing their output: any change to
what a detector reports shows up as a snapshot diff that a human must review.

## Layout

```
tests/
├── detector_snapshots.rs            # one #[test] per detector
├── gallery_snapshots.rs             # full registry over the bug gallery
├── fixtures/detectors/<name>.rs     # a focused fixture that trips <name>
├── fixtures/gallery/<bug>_*.rs      # canonical vulnerable + fixed corpus
└── snapshots/                       # reviewed golden output (committed)
    ├── detector_snapshots__<name>.snap
    └── gallery_snapshots__<bug>_*.snap
```

`detector_snapshots.rs` runs a single detector against its fixture and asserts
the resulting `Vec<RuleViolation>` with `insta::assert_yaml_snapshot!`. The
fixtures intentionally also contain *clean* code paths, so the snapshot proves
both what the detector flags **and** what it correctly leaves alone.

`gallery_snapshots.rs` runs the **full** default `RuleRegistry` over the
[canonical vulnerable-contract gallery](fixtures/gallery/README.md) — ten bug
classes, each as a vulnerable + fixed pair — so the shared corpus is wired into
the snapshot suite. See that gallery README for the bug-class → finding-code map.

## Running

```bash
# Run the detector snapshots (part of the normal suite too):
cargo test -p sanctifier-core --all-features --test detector_snapshots

# Or, with the insta runner (nicer output, used in CI):
cargo insta test -p sanctifier-core --all-features
```

When a detector's output changes, the test **fails** and `insta` writes a
pending `*.snap.new` file next to the existing snapshot.

## Reviewing changes

Install the helper once: `cargo install cargo-insta`.

```bash
# Interactively accept/reject each pending change:
cargo insta review

# Accept everything pending (only after eyeballing the diff):
cargo insta accept

# Throw away all pending changes:
cargo insta reject
```

Always read the diff. A snapshot change means a detector now reports something
different — make sure that difference is intended before accepting, then commit
the updated `.snap` file alongside your code change.

## Adding a detector

1. Add a fixture at `fixtures/detectors/<name>.rs` that triggers the detector
   (and ideally a clean path it must ignore). It only needs to parse as Rust —
   detectors analyze source with `syn`, they do not compile it.
2. Add a `#[test]` in `detector_snapshots.rs` calling `assert_detector_snapshot`.
3. Run `cargo insta test -p sanctifier-core --all-features`, then
   `cargo insta review` to accept the new snapshot.
4. Commit the fixture, the test, and the generated `.snap`.

## CI

CI runs `cargo insta test -p sanctifier-core --all-features --check --unreferenced reject`:

- `--check` fails the build on any snapshot diff (and never writes files), so
  unreviewed changes cannot merge.
- `--unreferenced reject` fails if a `.snap` is left behind with no matching
  test, keeping the snapshot set tidy.
