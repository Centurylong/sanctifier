# `error_repr_instability` — Stable error-code ABI across versions

| | |
| --- | --- |
| **Finding code** | [`SANCT_ERROR_REPR_INSTABILITY`](../error-codes.md) |
| **Category** | api_compatibility |
| **Severity** | Error |
| **Source rule** | [`rules/error_repr_instability.rs`](../../tooling/sanctifier-core/src/rules/error_repr_instability.rs) |
| **Glossary** | [Contract error](../glossary.md) |

## What it catches

Existing `#[contracterror]` variants whose `#[repr(u32)]` discriminants no
longer match the values captured by `sanctifier baseline`. Clients decode
contract failures by numeric error code, so reordering implicit variants or
renumbering explicit variants can silently change the public error ABI.

The detector intentionally requires a stored baseline. Without historical ABI
state there is no safe way to distinguish a new enum layout from a breaking
change.

## Vulnerable example

Baseline version:

```rust
#[contracterror]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    InvalidInput,
    Unauthorized,
}
```

Later version:

```rust
#[contracterror]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    Unauthorized, // was 3, now 2
    InvalidInput, // was 2, now 3
}
```

## The fix

Treat discriminants as part of the contract API. Preserve every existing value
and append new codes without renumbering earlier variants:

```rust
#[contracterror]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    InvalidInput = 2,
    Unauthorized = 3,
    NewFailure = 4,
}
```

Run `sanctifier baseline --update` only after an intentional compatibility
decision; the baseline file stores the error enum, variant name, and resolved
numeric discriminant alongside normal finding-suppression entries.

## How Sanctifier detects it

The baseline command resolves explicit and implicit `u32` discriminants for
`#[contracterror]` + `#[repr(u32)]` enums and stores them project-relative.
`sanctifier analyze` supplies this stored snapshot to the detector for each file
and includes ABI drift in text/JSON reports and the JSON failure status.
`--no-baseline` disables this comparison along with finding suppression.
During analysis, the rule compares variants with the same file, enum, and name
against that snapshot. A changed numeric value is an Error finding. Pure source
reordering is allowed when explicit discriminants remain unchanged.

**Limitations:** discriminants expressed as non-literal const expressions are
skipped because evaluating arbitrary Rust constants is outside static-source
scope.

## References

- Soroban docs — [Errors](https://soroban.stellar.org/docs/fundamentals-and-concepts/errors-and-panics)
- Related: [`error_code_collision`](error_code_collision.md)
