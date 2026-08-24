# `public_input_range` — Verification consumes public inputs without a field-element range check

| | |
| --- | --- |
| **Finding code** | [`SANCT_PUBLIC_INPUT_UNVALIDATED`](../error-codes.md) |
| **Category** | zk_verification |
| **Severity** | Error |
| **Source rule** | [`rules/public_input_range.rs`](../../tooling/sanctifier-core/src/rules/public_input_range.rs) |
| **Glossary** | [Public input](../glossary.md#public-input) · [Proof](../glossary.md#proof) |

## What it catches

A `#[contractimpl]` entrypoint that takes caller-supplied public inputs (a
parameter named `public_input(s)`, `pub_input(s)`, `public_signal(s)`,
`pub_signals`, `inputs`, or `instance(s)`) and feeds them into a verification
call — anything whose name contains `verify`, `pairing`, or `check_proof` —
**without first checking that every one of them is a canonical field element
strictly less than the scalar field modulus `r`**.

This is a different axis from [`proof_length_check`](proof_length_check.md).
That detector asks whether the *right number of bytes* arrived; this one asks
whether those bytes *denote a legal field element*. A buffer can be exactly the
expected length and still carry a value the verifier's soundness argument does
not cover.

## Why it matters

Every SNARK verifier's soundness proof assumes each public input is a canonical
element of `F_r`. Handing it something else buys an attacker two things:

**Aliasing.** Field arithmetic is modular, so `x` and `x + r` are the same
element. If the contract's own business logic reads a public input as an
ordinary integer — an amount, an account id, a nullifier — while the pairing
check sees it reduced mod `r`, the two disagree about what was proven. The
proof verifies honestly for `x mod r`; the contract acts on `x`. Nothing in the
verifier is broken, and nothing in the circuit is broken. The gap is entirely
in the unvalidated boundary between them.

**Implementation-defined reduction.** What a backend does with an out-of-range
limb is not part of any proof system's security argument. arkworks' unchecked
constructors keep the value as-is, some bellman paths reduce silently, and
hand-rolled assembly may do neither. A verifier that behaves correctly today
because of which crate it happens to link is relying on undefined behaviour,
and a dependency bump changes the answer.

Non-canonical *encodings* are the same problem in encoding space: two byte
strings that decode to the same element let an attacker produce a second,
distinct-looking submission for a nullifier or commitment that was supposed to
be spent exactly once.

## Vulnerable example

```rust
#[contractimpl]
impl Verifier {
    pub fn verify_claim(env: Env, proof: Bytes, public_inputs: Vec<u256>) -> bool {
        let vk = load_vk(&env);
        // public_inputs came straight off the wire. Nothing here established
        // that each element is < r, or that its encoding is canonical.
        Groth16::verify(&vk, &public_inputs, &proof)
    }
}
```

## The fix

Reject anything out of range before verifying. Any of these forms satisfies the
detector, because each one actually establishes the property:

```rust
// 1. Explicit comparison against the modulus.
for input in public_inputs.iter() {
    if input >= FR_MODULUS {
        return false;
    }
}

// 2. A checked deserializer that fails on a non-canonical encoding.
let elements = match Fr::from_canonical_bytes(&public_inputs) {
    Some(elements) => elements,
    None => return false,
};

// 3. A dedicated validation helper.
if !validate_public_inputs(&pub_signals) {
    return false;
}
```

Prefer failing closed with a typed `#[contracterror]` over returning `false`,
so a caller cannot confuse "the proof was invalid" with "the input was
malformed".

## What it does *not* flag

- Entrypoints that take public inputs but never verify anything — a setter is
  not making a soundness claim.
- Verification with no public-input parameter at all.
- Functions inside `#[cfg(test)]` modules.
- A line carrying `// sanctifier:ignore[SANCT_PUBLIC_INPUT_UNVALIDATED]`.

Note that `*_unchecked` constructors are deliberately **not** treated as
validation, even when their name contains `canonical`
(`from_canonical_bytes_unchecked` is still flagged). That naming is exactly
where this bug tends to hide.

## References

- [ZKSecurity — *Zero-Knowledge Proof Vulnerabilities: Missing Input Validation*](https://zksecurity.xyz/blog/posts/zksecurity-common-vulnerabilities/)
- [0xPARC — *ZK Bug Tracker*, "Unchecked public inputs" and "Missing range checks"](https://github.com/0xPARC/zk-bug-tracker)
- [Trail of Bits — *Breaking the Shield: Bugs in ZK verifiers*](https://blog.trailofbits.com/2022/04/13/part-1-coordinated-disclosure-of-vulnerabilities-affecting-girault-bulletproofs-and-plonk/)
- [arkworks `PrimeField::from_bigint`](https://docs.rs/ark-ff/latest/ark_ff/fields/trait.PrimeField.html#tymethod.from_bigint) — returns `None` for values `>= modulus`; the unchecked variants do not
- [`verifier-checklist.md`](../verifier-checklist.md) — the full integration checklist this detector implements one row of
