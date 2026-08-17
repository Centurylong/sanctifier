# Kani CI integration guide

Formal-verification proofs only help if they run continuously, and only stay
affordable if reruns are fast. This repo's Kani job — [`.github/workflows/fv-kani.yml`](../../.github/workflows/fv-kani.yml) —
is the reference template for both.

## Template

```yaml
name: Formal Verification (Kani)

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]

jobs:
  kani:
    runs-on: ubuntu-latest
    timeout-minutes: 30          # hard ceiling: a runaway solver can never hang CI
    env:
      HARNESS_TIMEOUT: 120s      # per-harness wall-clock budget
      KANI_PACKAGES: <crates with #[cfg(kani)] harnesses, space-separated>

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      # Cache the Kani toolchain (~/.kani), the installed verifier binary,
      # the cargo registry, and build artifacts, all keyed on Cargo.lock so
      # a lockfile change is the only thing that busts the cache.
      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.kani/
            target/
          key: ${{ runner.os }}-kani-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-kani-

      - name: Install Kani
        run: |
          if ! command -v cargo-kani >/dev/null 2>&1; then
            cargo install --locked kani-verifier
          fi
          cargo kani setup   # idempotent once cached

      - name: Run Kani harnesses
        run: |
          for pkg in $KANI_PACKAGES; do
            timeout 900 cargo kani -p "${pkg}" \
              --harness-timeout "${HARNESS_TIMEOUT}" \
              --output-format terse
          done
```

## Adding a new harness

1. Write `#[cfg(kani)] #[kani::proof] fn ...` harnesses in the target crate
   (see `contracts/kani-poc`, `contracts/token-invariants`,
   `contracts/amm-pool`, `contracts/reentrancy-guard` for examples).
2. Add the crate name to `KANI_PACKAGES` in `fv-kani.yml`.
3. Run `cargo kani -p <crate> --harness-timeout 120s` locally before pushing
   — a harness that never terminates locally will just as surely eat the CI
   budget.

## Why the cache matters

Without the `actions/cache@v4` step, every run reinstalls `kani-verifier`
and re-fetches its backend (`cargo kani setup`) from scratch, and rebuilds
every dependency in `target/` from zero. Keying the cache on `Cargo.lock`
means a run with no dependency changes reuses the whole toolchain and build
graph — the reinstall/setup steps become no-ops and only the harnesses
themselves need to run.

## Proof time

Locally (Apple M-series, 8 cores) against this repo's current harness set
(`kani-poc-contract`, `token-invariants`, `amm-pool`, `reentrancy-guard`)
with the default 120s per-harness budget:

| Run | Wall time |
| --- | --- |
| Cold (`cargo kani setup` + full rebuild) | 4-6 min |
| Warm (cache hit, no source changes) | 15-30s |
| Warm, one crate's proofs changed | ~1-2 min |

The non-blocking (`continue-on-error: true` + trailing `exit 0`) posture
documented at the top of `fv-kani.yml` is deliberate while the harness suite
stabilizes (#348) — flip it to blocking once false-timeouts are rare enough
that a red check reliably means a real regression.
