# Secure Soroban Patterns Cookbook

Vetted, correct-by-construction patterns for common Soroban contract
concerns. Each entry pairs the pattern with the Sanctifier detector(s) that
enforce it, so a finding always points back to guidance instead of just a
rule name. For the full walkthrough of getting a project to a passing CI
gate around these detectors, see [`docs/tutorial.md`](tutorial.md).

## Authorization

**Pattern:** every state-mutating entrypoint calls `.require_auth()` on the
address whose permission the call actually depends on, *before* the mutation
happens — not on an unrelated address, and not only in a read-only path.

```rust
pub fn withdraw(env: Env, owner: Address, amount: i128) {
    owner.require_auth();
    // ...mutate storage only after the check above...
}
```

**Detector:** [`auth_gap`](detectors/auth_gap.md) flags entrypoints that
mutate contract state without an auth check on the relevant path.
[`wrong_auth_args`](detectors/wrong_auth_args.md) flags `require_auth()`
called on the *wrong* address (e.g. checking `admin` when the operation is
scoped to `owner`).

## Visibility

**Pattern:** don't expose internal helper functions as `pub fn` on a
`#[contractimpl]` block unless they're meant to be externally callable —
every `pub fn` there becomes a contract entrypoint.

**Detector:** [`sanct_visibility`](detectors/sanct_visibility.md).

## Storage & TTL

**Pattern:** every persistent storage entry gets its TTL bumped on both read
and write (`extend_ttl`), with a threshold comfortably below the extend
target, so actively-used data is never archived out from under its owner.

```rust
env.storage().persistent().set(&key, &value);
env.storage()
    .persistent()
    .extend_ttl(&key, THRESHOLD_LEDGERS, EXTEND_LEDGERS);
```

**Detector:** [`missing_ttl`](detectors/missing_ttl.md) flags persistent
writes with no corresponding TTL extension.
[`ledger_size`](detectors/ledger_size.md) flags storage structs likely to
exceed the ledger entry size limit before TTL even becomes relevant.

## Admin & Upgrades

**Pattern:** never hardcode an admin/owner address as a literal — store it
in contract data during `initialize`, and gate the upgrade entrypoint behind
`require_auth()` on that stored admin, not a compile-time constant.

**Detector:**
[`init_hardcoded_admin`](detectors/init_hardcoded_admin.md) flags
initialization that bakes in a fixed admin address.
[`hardcoded_addr`](detectors/hardcoded_addr.md) flags hardcoded addresses
more generally (fee recipients, oracles, etc. that should be configurable).

## Arithmetic Safety

**Pattern:** use checked arithmetic (`checked_add`, `checked_sub`,
`checked_mul`) or an explicit overflow-safe type on every balance/amount
computation; never rely on Rust's debug-only overflow panics being present
in a release WASM build.

**Detector:**
[`arithmetic_overflow`](detectors/arithmetic_overflow.md) flags raw
`+`/`-`/`*` on values that look like balances or amounts.
[`unsigned_underflow`](detectors/unsigned_underflow.md) flags unsigned
subtraction that can wrap instead of erroring.
[`shift_overflow`](detectors/shift_overflow.md) flags bit-shifts that can
overflow the operand width.
[`division_by_zero`](detectors/division_by_zero.md) flags divisions without
a preceding zero-check.

## Error Handling

**Pattern:** propagate `Result`s with `?` or an explicit match — don't
`.unwrap()` a fallible call in a way that turns a recoverable error into an
unconditional panic for every caller.

**Detector:** [`sanct_unwrap`](detectors/sanct_unwrap.md) and
[`eager_unwrap_or`](detectors/eager_unwrap_or.md) flag `.unwrap()` /
`.unwrap_or(...)` on `Result`/`Option` in non-test code.
[`panic_detection`](detectors/panic_detection.md) flags explicit `panic!`
in entrypoint logic. [`unhandled_result`](detectors/unhandled_result.md)
flags a `Result`-returning call whose error is silently dropped.

## Read-Only Views Stay Read-Only

**Pattern:** a view/query function must never write to storage — callers
(and off-chain simulators) assume view calls are side-effect-free and may
call them without submitting a transaction at all.

**Detector:**
[`state_write_in_view`](detectors/state_write_in_view.md).

## Reentrancy & Cross-Contract Calls

**Pattern:** finish all of *your* state mutations before invoking another
contract (`env.invoke_contract`), so a malicious or buggy callee can't
re-enter you mid-update and observe inconsistent state. See the
[confused-deputy and reentrancy case studies](case-studies/admin-takeover.md)
for a worked example of the failure mode this prevents.

## Randomness & Commitments

**Pattern:** never derive anything security-relevant (auction winners,
lottery outcomes, tie-breaks) from ledger timestamp/sequence alone — they're
influenced by the block producer. Use a commit-reveal scheme or an oracle
with its own staleness checks instead.

See the weak-randomness and oracle-staleness fixtures under
`tooling/sanctifier-core/tests/fixtures/gallery/` for a vulnerable/fixed
pair demonstrating this end to end.

---

Every pattern above is backed by a detector with a fixture and a golden
snapshot test — see
[`tooling/sanctifier-core/tests/detector_snapshots.rs`](../tooling/sanctifier-core/tests/detector_snapshots.rs)
if you want to see the exact code each rule catches.
