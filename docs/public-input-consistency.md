# Circuit-vs-Contract Public-Input Consistency (issue #740)

This is a feasibility write-up and prototype, not a finished detector — issue
#740 is filed as `type: research`, scoped to "investigate feasibility +
approach, prototype flags a mismatch." This page covers both.

## The problem

A ZK proof is checked against a specific ordered list of **public inputs**.
The circuit (`tooling/zk/src/circuit.rs`) declares them by calling
`FpVar::new_input(...)` once per field, in a fixed order documented in its
header comment:

```rust
// # Public inputs (in allocation order — must match the verifier)
// 1. `wasm_hash`
// 2. `ruleset_version`
// 3. `score_threshold`
// 4. `rules_commitment`
```

The Soroban verifier contract (`contracts/zk-verifier/src/lib.rs::verify`)
never sees the circuit — it just deserializes a flat `Bytes` blob:

```rust
if public_inputs_bytes.len() != 4 * 32 {
    return false;
}
// ...
let mut inputs = [Fr::from(0u8); 4];
for i in 0..4 {
    let start = i * 32;
    // ...
}
```

Nothing ties that `4` (or the assumption that inputs are laid out in
allocation order) to the circuit's actual definition. If someone adds a 5th
public input to the circuit and forgets the contract, or reorders two fields,
`verify()` keeps compiling and keeps returning `true`/`false` — it just
silently checks proofs against the wrong statement. That's a soundness bug
with no compiler error and no obvious symptom short of an audit or an
incident.

## Feasibility

**Count consistency is fully checkable statically, today**, with the same
technique every other Sanctifier rule already uses: parse both files with
`syn`, count structural signals, compare. No circuit execution, no proving
key, no dependency on `arkworks` from `sanctifier-core` — this is a pure
source-text comparison, implemented in
[`public_input_consistency.rs`](../tooling/sanctifier-core/src/public_input_consistency.rs).

- **Circuit side**: count `..::new_input(...)` call sites. Each one
  allocates exactly one public input (`ark-r1cs-std`'s `AllocVar::new_input`
  convention), in call order.
- **Contract side**: find a `<count> * 32` pattern (32 bytes = one compressed
  BLS12-381 `Fr` element) — the shape this repo's verifier uses to validate
  the incoming byte length before deserializing.
- **Compare** the two counts; mismatch is a hard error.

**Order/encoding consistency is out of scope for this pass**, and that's a
deliberate, not accidental, limitation:

- The circuit's public-input *names* only exist as Rust field names
  (`AuditPublicInputs { wasm_hash, ruleset_version, ... }`) and doc-comment
  prose — nothing machine-checkable ties allocation order to those names
  beyond convention.
- The contract has no per-field structure at all; it's a flat byte
  loop (`for i in 0..4 { ... }`), so there's no "field 3" to compare against
  "the circuit's 3rd input" — only a count.
- Checking *order* soundly would require the circuit to export a structured,
  ordered manifest of its public inputs (names + types) rather than relying
  on convention. That's a real, buildable follow-up: derive a small
  `#[public_inputs(wasm_hash, ruleset_version, score_threshold,
  rules_commitment)]`-style attribute (or a `pub const PUBLIC_INPUT_ORDER: &[&str]`
  in the circuit crate) that both a codegen step and a static check like this
  one could read — but it means changing how circuits declare their public
  inputs, not just adding a checker. That's a bigger, separate change than
  this issue's scope.

## Prototype

`sanctifier check-public-inputs --circuit <path> --contract <path>` runs the
comparison and exits non-zero on a mismatch, so it's usable as a CI gate
alongside `sanctifier prove`/`sanctifier verify`:

```bash
$ sanctifier check-public-inputs \
    --circuit tooling/zk/src/circuit.rs \
    --contract contracts/zk-verifier/src/lib.rs
✅ circuit and contract agree on 4 public input(s).
```

The prototype is exercised against this repo's own circuit and contract in
`public_input_consistency.rs`'s test suite
(`the_repos_own_circuit_and_contract_currently_agree`), and a synthetic case
proves it actually catches drift
(`flags_a_mismatch_when_circuit_drops_a_public_input`): removing one
`new_input` call from a copy of the circuit while leaving the contract's
`4 * 32` check untouched produces:

```
❌ MISMATCH: circuit declares 3 public input(s) (via new_input calls) but the
   contract assumes 4 (via a `N * 32`-byte length check). A verifier built
   against one will reject — or worse, silently misparse — proofs shaped for
   the other.
```

## Recommended follow-up (not in this PR)

Wire `check-public-inputs` into CI (`.github/workflows/ci.yml`) as a step
alongside the existing `sanctifier prove` SMT check, so a circuit/contract
drift fails the build the same way a broken invariant does. Left out here
since it's a CI-workflow change with its own review surface, separate from
landing the checker itself.
