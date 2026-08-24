# `bls_subgroup_check` — BLS12-381 point reaches a pairing without a subgroup check

| | |
| --- | --- |
| **Finding code** | [`SANCT_BLS_SUBGROUP_UNCHECKED`](../error-codes.md) |
| **Category** | cryptography |
| **Severity** | Error |
| **Source rule** | [`rules/bls_subgroup_check.rs`](../../tooling/sanctifier-core/src/rules/bls_subgroup_check.rs) |
| **Glossary** | [Proof](../glossary.md#proof) · [Pairing](../glossary.md#pairing) |

## What it catches

In a file that uses BLS12-381, a function that reaches a pairing call
(`pairing`, `miller_loop`, `final_exponentiation`, or any `verify`) with a
proof point that nothing has checked for subgroup membership. Two shapes:

1. **An `_unchecked` constructor or deserializer** —
   `deserialize_uncompressed_unchecked`, `from_compressed_unchecked`,
   `new_unchecked`, and friends. In arkworks these are precisely the entry
   points that skip the subgroup check.
2. **A point arriving as a typed parameter** — `G1Affine`, `G2Affine`,
   `G1Projective`, `G2Projective`. Deserialization happened elsewhere, so
   nothing in this function establishes membership.

## Why it matters

The pairing `e: G1 × G2 → GT` is defined on the **prime-order subgroups**, not
on the full curve. BLS12-381 has a cofactor in both groups — `h₁ ≈ 2⁶⁴` in G1
and `h₂ ≈ 2^318` in G2 — so the curve contains plenty of points outside the
subgroup a proof system reasons about. Feeding one to the pairing evaluates it
outside the domain the soundness argument covers, and two things follow:

**Malleability.** Given a valid proof point `P`, an attacker can often produce
`P + T` for a small-order `T` such that the verification equation still holds.
The proof serializes differently but verifies identically, so any replay
defence keyed on the proof bytes — a "this proof was already used" set, a
nullifier derived from the encoding — is bypassed with a fresh-looking
submission.

**Forgery.** Off-subgroup, the algebraic relations the verifier checks no
longer pin the witness down. This is the mechanism behind the small-subgroup
attacks that motivated mandatory subgroup checks in the BLS signature standard,
and the same reasoning applies to any pairing-based proof verifier.

The check is not free — it is a scalar multiplication per point — which is
exactly why implementations reach for the `_unchecked` variants and why this
bug survives review. Skipping the check is safe only for points the contract
itself produced, never for anything a caller supplied.

## Vulnerable example

```rust
use ark_bls12_381::{Bls12_381, G1Affine, G2Affine};

fn verify_proof(proof_bytes: &[u8], vk: &VerifyingKey) -> bool {
    // `_unchecked` skips the subgroup check by design, and nothing here
    // performs it afterwards.
    let a = G1Affine::deserialize_uncompressed_unchecked(proof_bytes).unwrap();
    let b = G2Affine::deserialize_uncompressed_unchecked(proof_bytes).unwrap();
    Bls12_381::pairing(a, b) == vk.alpha_beta
}
```

## The fix

Either use the checked deserializer, or keep the fast path and do the check
explicitly:

```rust
let a = G1Affine::deserialize_uncompressed_unchecked(proof_bytes)?;
if !a.is_on_curve() || !a.is_in_correct_subgroup_assuming_on_curve() {
    return Err(Error::InvalidProofPoint);
}
```

Both conditions are needed: `is_in_correct_subgroup_assuming_on_curve`, as its
name says, assumes the point is on the curve to begin with. Mapping into the
subgroup with `clear_cofactor()` / `mul_by_cofactor()` is also accepted by the
detector where rejecting is not appropriate.

## What it does *not* flag

- Files that do not use BLS12-381 at all.
- BLS code that never reaches a pairing — deserializing a point for storage
  makes no verification claim.
- Any function where a membership check is present.
- `#[cfg(test)]` modules, and lines carrying
  `// sanctifier:ignore[SANCT_BLS_SUBGROUP_UNCHECKED]`.

## References

- [RFC 9380 §2.1 / IRTF CFRG BLS signatures draft, "Subgroup checks"](https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-bls-signature-05#section-5.2) — mandatory `KeyValidate` / subgroup validation and why
- [Sean Bowe — *BLS12-381: New zk-SNARK Elliptic Curve Construction*](https://electriccoin.co/blog/new-snark-curve/) — cofactor structure of G1 and G2
- [0xPARC — *ZK Bug Tracker*: "Missing curve/subgroup checks"](https://github.com/0xPARC/zk-bug-tracker)
- [Trail of Bits — *Breaking the Shield*](https://blog.trailofbits.com/2022/04/13/part-1-coordinated-disclosure-of-vulnerabilities-affecting-girault-bulletproofs-and-plonk/) — proof malleability from unchecked group elements
- [arkworks `CanonicalDeserialize` — `Validate::Yes` vs `Validate::No`](https://docs.rs/ark-serialize/latest/ark_serialize/enum.Validate.html) — what the `_unchecked` variants actually skip
- [`verifier-checklist.md`](../verifier-checklist.md) — the wider verifier integration checklist
