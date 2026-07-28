# `error_code_collision` — Duplicate `#[contracterror]` discriminants

| | |
| --- | --- |
| **Finding code** | [`S016`](../error-codes.md) |
| **Category** | code_hygiene |
| **Severity** | Medium |
| **Source rule** | [`rules/error_code_collision.rs`](../../tooling/sanctifier-core/src/rules/error_code_collision.rs) |
| **Glossary** | [Contract error](../glossary.md) |

## What it catches

Two variants of a `#[contracterror]` enum that share the same integer
discriminant, or a discriminant sequence that is inconsistent (gaps mixed with
explicit values in a way that invites collisions). Callers and off-chain
tooling identify errors by their **numeric code**; a collision makes two distinct
failures indistinguishable, breaking error handling and incident triage.

## Vulnerable example

```rust
#[contracterror]
#[derive(Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    InsufficientBalance = 2,
    Expired = 2, // collision: same code as InsufficientBalance
}
```

## The fix

Give every variant a unique, stable discriminant:

```rust
#[contracterror]
#[derive(Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    InsufficientBalance = 2,
    Expired = 3,
}
```

Treat error numbers as an API: never renumber an existing variant, only append.

## How Sanctifier detects it

The rule parses `#[contracterror]` enums, resolves each variant's explicit or
implicit discriminant, and reports any duplicates or inconsistent numbering.

**Limitations:** discriminants computed via `const` expressions the rule cannot
evaluate are treated conservatively.

## References

- Soroban docs — [Errors](https://soroban.stellar.org/docs/fundamentals-and-concepts/errors-and-panics)
- [CWE-1078: Inappropriate Source Code Style or Formatting](https://cwe.mitre.org/data/definitions/1078.html)
- Related: [`unhandled_result`](unhandled_result.md)
