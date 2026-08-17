# ZK verifier support matrix

Not every Soroban contract that verifies a proof uses Groth16. PLONK and
Halo2 are increasingly common precisely because they avoid a per-circuit
trusted-setup ceremony. `sanctifier_zk::verifier_patterns` (in
[`tooling/zk/src/verifier_patterns.rs`](../tooling/zk/src/verifier_patterns.rs))
gives the rest of the toolchain a shared, lightweight way to recognize which
proof system a verifier contract's source is built on, from its crate
imports and type paths (`ark_groth16::*`, `ark_plonk::*` / `plonky2`,
`halo2_proofs::*` / `halo2curves`).

## Support matrix

| Proof system | Trusted setup | Detected via | Source-analysis coverage |
| --- | --- | --- | --- |
| Groth16 (BLS12-381) | Per-circuit | `ark_groth16`, `Groth16::`, `VerifyingKey` | Full — reference contract (`contracts/zk-verifier`), all `docs/detectors/*` heuristics apply (e.g. [`proof_length_check`](detectors/proof_length_check.md)) |
| PLONK | Universal (one ceremony, reusable across circuits) | `ark_plonk`, `plonky2`, `PlonkVerifier`, `plonk::` | Pattern-recognized; general heuristics (proof/public-input naming, verify-call shape) apply the same way since they're crate-agnostic |
| Halo2 | Transparent (no ceremony) | `halo2_proofs`, `halo2curves`, `halo2_gadgets` | Pattern-recognized; same general heuristics apply |

`sanctifier_core`'s detectors (e.g. `proof_length_check`, which flags a
verify call reached without a length check on the proof/public-input bytes)
operate on parameter names and call shapes, not on which crate the `verify`
call resolves to — so they already apply to PLONK/Halo2 verifiers as-is.
`detect_proof_systems` exists so tooling that *does* need to branch per
system (reports, docs generation, future system-specific detectors) has one
shared classifier instead of every caller re-implementing crate-name
matching.

## Usage

```rust
use sanctifier_zk::verifier_patterns::{detect_proof_systems, ProofSystem};

let systems = detect_proof_systems(contract_source);
for system in &systems {
    println!("{}: trusted setup required = {}", system.as_str(), system.requires_trusted_setup());
}
```

## Extending

Adding a new proof system: add its crate-name/type markers as a new `&[&str]`
constant in `verifier_patterns.rs`, add the `ProofSystem` variant, and add a
row to the table above.
