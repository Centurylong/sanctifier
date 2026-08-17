# Reference-safe verifier template

[`contracts/zk-verifier/src/lib.rs`](../contracts/zk-verifier/src/lib.rs) is
the golden reference for a Soroban ZK-proof verifier contract: a hardened
starting point new verifier contracts can be diffed against (see the
[Verifier Checklist](verifier-checklist.md) for what to check line-by-line).

## Annotated walkthrough

```rust
#[contractimpl]
impl ZkVerifierContract {
    pub fn init(env: Env, vk_bytes: Bytes) {
        // (1) Guards re-initialization instead of silently overwriting a
        //     live verifying key.
        if env.storage().instance().has(&DataKey::VerifyingKey) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::VerifyingKey, &vk_bytes);
    }

    pub fn verify(env: Env, proof_bytes: Bytes, public_inputs_bytes: Bytes) -> bool {
        let vk_bytes: Bytes = env
            .storage()
            .instance()
            .get(&DataKey::VerifyingKey)
            .expect("Not initialized");

        // (2) Every caller-supplied byte array is length-checked against its
        //     fixed-size buffer *before* copy_into_slice — this is exactly
        //     what the `proof_length_check` detector looks for.
        let mut vk_slice = [0u8; 1024];
        let vk_len = vk_bytes.len() as usize;
        if vk_len > vk_slice.len() {
            return false;
        }
        vk_bytes.copy_into_slice(&mut vk_slice[..vk_len]);

        let mut proof_slice = [0u8; 512];
        let proof_len = proof_bytes.len() as usize;
        if proof_len > proof_slice.len() {
            return false;
        }
        proof_bytes.copy_into_slice(&mut proof_slice[..proof_len]);

        // (3) Deserialization failures return `false` (a clean rejection)
        //     instead of panicking or unwrapping.
        let vk = match VerifyingKey::<Bls12_381>::deserialize_compressed(&vk_slice[..vk_len]) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let proof = match Proof::<Bls12_381>::deserialize_compressed(&proof_slice[..proof_len]) {
            Ok(p) => p,
            Err(_) => return false,
        };

        // (4) Public-input length is checked against the *exact* expected
        //     size for this circuit (4 field elements * 32 bytes each) —
        //     not just "does it fit the buffer".
        if public_inputs_bytes.len() != 4 * 32 {
            return false;
        }

        let mut inputs_slice = [0u8; 128];
        public_inputs_bytes.copy_into_slice(&mut inputs_slice);
        let mut inputs = [Fr::from(0u8); 4];
        for i in 0..4 {
            let start = i * 32;
            let end = start + 32;
            inputs[i] = match Fr::deserialize_compressed(&inputs_slice[start..end]) {
                Ok(f) => f,
                Err(_) => return false,
            };
        }

        let pvk = Groth16::<Bls12_381>::process_vk(&vk).unwrap();
        Groth16::<Bls12_381>::verify_with_processed_vk(&pvk, &inputs, &proof).unwrap_or(false)
    }
}
```

This template is Groth16-specific (`ark_groth16`, `Bls12_381`), but the
structural properties it demonstrates — init-guard, length-check-before-copy,
match-not-unwrap on deserialization, exact-size check on public inputs — are
proof-system agnostic and apply the same way to a PLONK or Halo2 verifier
(see the [ZK verifier support matrix](zk-verifier-support.md)).

## See also

- [Verifier Checklist](verifier-checklist.md) — the diffable, detector-mapped
  checklist derived from this template.
- [`proof_length_check`](detectors/proof_length_check.md) — the detector that
  flags point (2)/(4) violations.
