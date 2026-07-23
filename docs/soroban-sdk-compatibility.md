# soroban-sdk Compatibility

Sanctifier analyses contracts written against a range of `soroban-sdk`
releases. To catch regressions early, CI builds each supported version in an
isolated probe crate on every push and pull request.

## Supported versions

| soroban-sdk | Status     | Notes |
| ----------- | ---------- | ----- |
| `20.5.0`    | ✅ primary | Version the workspace pins (`Cargo.toml`). All detectors and snapshots are calibrated against it. |
| `20.4.0`    | ✅ tested  | Builds cleanly on the pinned toolchain. |
| `20.3.0`    | ✅ tested  | Lowest 20.x release exercised in CI; `soroban-env-common 20.3.0` pulls `ethnum 1.5.0`, which drives the toolchain pin below. |

The matrix lives in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)
(job `soroban-sdk-compat`) and is driven by
[`scripts/soroban-sdk-compat.sh`](../scripts/soroban-sdk-compat.sh). Run it
locally with:

```bash
rustup run 1.85.0 ./scripts/soroban-sdk-compat.sh 20.4.0
```

## Version-specific handling

- **Toolchain pin (all 20.x).** The transitive dependency `ethnum 1.5.0` (via
  `soroban-env-common 20.3.0`) fails to compile on newer `rustc` (an `E0512`
  transmute size check became a hard error), and `base64ct 1.8.3` needs the
  `edition2024` feature stabilised in Rust 1.85. CI therefore pins
  `dtolnay/rust-toolchain@1.85.0` — the lowest stable that satisfies both. Do
  not bump the toolchain without re-running the compatibility matrix.

- **Isolated probe, not a lockfile bump.** The workspace declares a single
  `soroban-sdk = { version = "20.5.0" }` requirement, so `cargo update
  --precise` cannot downgrade it within the workspace. The compatibility job
  instead builds a throwaway crate pinned to `=<version>`, keeping the check
  hermetic and the main build's resolution untouched.

- **Detector calibration.** Golden snapshots and the differential corpus are
  fixed against `20.5.0`. The compatibility matrix only asserts that each
  version *compiles*; it does not re-run the detector snapshots, which remain
  bound to the primary version.

## Adding a new version

1. Confirm it builds locally: `rustup run 1.85.0 ./scripts/soroban-sdk-compat.sh <version>`.
2. Add it to the `matrix.soroban-sdk` list in `.github/workflows/ci.yml`.
3. Add a row to the table above.
4. If the primary version changes, update the workspace `Cargo.toml` pin and
   re-review the detector snapshots (`cargo insta review`).
