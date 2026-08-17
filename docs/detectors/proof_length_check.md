# `proof_length_check` — Verifier call reached without validating proof/input length

| | |
| --- | --- |
| **Finding code** | [`SANCT_PROOF_LENGTH_UNVALIDATED`](../error-codes.md) |
| **Category** | zk_verification |
| **Severity** | Warning |
| **Source rule** | [`rules/proof_length_check.rs`](../../tooling/sanctifier-core/src/rules/proof_length_check.rs) |
| **Glossary** | [Proof](../glossary.md#proof) · [Public input](../glossary.md#public-input) |

## What it catches

A `#[contractimpl]` entrypoint that takes a proof or public-input byte array
(a parameter named `proof`, `proof_bytes`, `public_input(s)`,
`public_input(s)_bytes`, or `inputs`) and passes it into a verifier call
(`.verify(...)`, `verify_with_processed_vk(...)`, or any call whose name
contains `verify`) **without checking that array's length anywhere in the
function first**.

Verifier contracts typically copy a caller-supplied `Bytes`/`Vec<u8>` into a
fixed-size buffer before deserializing a proof or public-input vector (see
`contracts/zk-verifier/src/lib.rs`). Skipping the length check either panics
on a truncated/oversized buffer or, if the copy silently truncates, lets a
malformed proof reach the verifier in a shape it was never meant to see —
turning an input-validation bug into a potential verification bypass.

## Vulnerable example

```rust
#[contractimpl]
impl Contract {
    // VULN: no length check on `proof_bytes` / `public_inputs_bytes` before
    // they are copied into fixed-size buffers and handed to the verifier.
    pub fn verify(env: Env, proof_bytes: Bytes, public_inputs_bytes: Bytes) -> bool {
        let mut proof_slice = [0u8; 512];
        proof_bytes.copy_into_slice(&mut proof_slice);
        inner_verify(&proof_slice, &public_inputs_bytes)
    }
}
```

## The fix

Validate the expected length(s) before touching the buffers:

```rust
#[contractimpl]
impl Contract {
    pub fn verify(env: Env, proof_bytes: Bytes, public_inputs_bytes: Bytes) -> bool {
        if proof_bytes.len() != 512 || public_inputs_bytes.len() != 4 * 32 {
            return false;
        }
        let mut proof_slice = [0u8; 512];
        proof_bytes.copy_into_slice(&mut proof_slice);
        inner_verify(&proof_slice, &public_inputs_bytes)
    }
}
```

## How Sanctifier detects it

The rule scans `#[contractimpl]` entrypoints for parameters whose name matches
common proof/public-input naming, then walks the function body for both a
`.len()` call on that parameter and a call whose name contains `verify`. If a
verify-like call is reached and none of the proof-like parameters were ever
checked with `.len()`, it's flagged. `#[cfg(test)]` modules and lines carrying
a `sanctifier:ignore[SANCT_PROOF_LENGTH_UNVALIDATED]` justification are
skipped.

Limitations: this is name- and call-shape heuristic, not a dataflow analysis —
it doesn't verify the length check actually runs *before* the verify call, and
a length check performed in a helper function called earlier in the chain is
not seen. A parameter with an unconventional name may be missed.

## References

- [Soroban docs — Errors and panics](https://soroban.stellar.org/docs/fundamentals/errors)
- [CWE-20: Improper Input Validation](https://cwe.mitre.org/data/definitions/20.html)
- Related: `view_panic`, `sanct_unwrap`
