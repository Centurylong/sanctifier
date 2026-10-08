# One-shot initialization proof — Sanctifier #339

The Soroban `TokenContract::initialize` entrypoint reads the instance-storage
`init` flag, calls `initialize_transition`, and persists the updated flag only
after success. The **same pure state-transition function** is verified by Kani.

## Focused proofs

From the workspace root:

```sh
cargo kani -p kani-poc-contract --harness init_at_most_once --harness-timeout 120s
cargo kani -p kani-poc-contract --harness init_bug_reinitialization_counterexample --harness-timeout 120s
```

- `init_at_most_once` starts from a **symbolic initialized flag**. If the
  contract was already initialized, the first attempt fails. If it is fresh,
  the first attempt succeeds and sets the flag. In either case, the second
  attempt fails and the flag remains set.
- `init_bug_reinitialization_counterexample` intentionally removes the
  initialization guard. Its second call succeeds when it must fail. Kani's
  `#[kani::should_panic]` asserts that the property detects this bug; a
  passing *expected-counterexample* result is not an endorsement of the buggy
  implementation.

The ordinary `initialize_transition_rejects_second_call` unit test provides
a concrete two-call witness without requiring Kani.

## CI wiring

`.github/workflows/fv-kani.yml` already includes `kani-poc-contract` in
`KANI_PACKAGES` and sets `HARNESS_TIMEOUT=120s` with a 30-minute job
ceiling. These two `#[kani::proof]` functions are part of that existing Kani
package discovery; a second workflow or an all-repository test sweep is
unnecessary. The upstream workflow is currently **non-blocking**, so review
the per-harness results rather than treating a green workflow job as proof.

## Verification boundary

This Kani model verifies the **pure guard and state transition** used by
production `TokenContract::initialize`. It does *not* model the Soroban
`Env`, `Address`, host storage persistence, transaction rollback, concurrency,
or runtime traps. Those Host/FFI behaviors require separate contract-level
execution or formal modeling. The proof must not be described as verifying
the full deployed Soroban contract. The intentionally buggy implementation
is compiled only under `cfg(kani)` or `cfg(test)`, never in a release build.
