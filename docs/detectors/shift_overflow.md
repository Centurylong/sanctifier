# `shift_overflow` — Bit shift by an amount that may exceed the operand width

| | |
| --- | --- |
| **Finding code** | [`SANCT_SHIFT_OVERFLOW`](../error-codes.md) |
| **Category** | arithmetic |
| **Severity** | Warning / Error |
| **Source rule** | [`rules/shift_overflow.rs`](../../tooling/sanctifier-core/src/rules/shift_overflow.rs) |
| **Glossary** | [Overflow](../glossary.md#overflow) |

## What it catches

A bit-shift (`<<`, `>>`, `<<=`, `>>=`) whose shift amount may be **greater than or
equal to the bit width** of the value being shifted. In Rust that is a bug: a shift
by `>= width` panics in debug builds and is undefined (masked) in release, so the
result is silently wrong. On Soroban that means a corrupted value or an aborted
invocation an attacker can trigger by supplying a large shift amount.

The detector distinguishes two cases:

- **Constant amount `>=` width** — provably wrong on every call. Reported as an
  **Error**.
- **Unbounded variable amount** — a caller-controlled shift with no visible guard.
  Reported as a **Warning**.

Amounts that are provably in range are ignored: a small constant, an amount masked
with `& mask` (or `% modulus`), or an amount guarded by a comparison (`if n < width`).

## Vulnerable example

```rust
#[contractimpl]
impl Packer {
    // VULN: `amount` is caller-controlled and never bounded; if amount >= 64 the
    // shift is undefined. Warning.
    pub fn pack(_env: Env, value: u64, amount: u32) -> u64 {
        value << amount
    }

    // VULN: 40 >= 32, so this constant shift is always wrong. Error.
    pub fn constant_overflow(_env: Env, value: u32) -> u32 {
        value << 40
    }
}
```

## The fix

Bound the shift amount — mask it, guard it with a comparison, or use the checked
shift APIs that return `None` on overflow:

```rust
#[contractimpl]
impl Packer {
    pub fn pack(_env: Env, value: u64, amount: u32) -> u64 {
        // Mask into range …
        value << (amount & 63)
    }

    pub fn pack_checked(_env: Env, value: u64, amount: u32) -> u64 {
        // … or reject out-of-range amounts explicitly.
        value.checked_shl(amount).unwrap_or(0)
    }
}
```

## How Sanctifier detects it

The rule walks each function, tracking operand bit widths (from signature parameter
types and typed `let` bindings) and the set of identifiers proven bounded within the
function. For every shift it classifies the amount as a constant, a bounded
expression (masked, modulo, or an ident guarded by a comparison), or unbounded, and
reports the constant-overflow and unbounded cases.

**Limitations:** it reasons about a single function — an amount bounded by logic in a
caller, or by a type invariant the rule can't see, is a false positive; add an
explicit mask or `if` guard, or switch to `checked_shl` / `checked_shr`.

## References

- Rust reference — [Arithmetic and logical binary operators](https://doc.rust-lang.org/reference/expressions/operator-expr.html#arithmetic-and-logical-binary-operators) (shift overflow is undefined)
- [`u64::checked_shl`](https://doc.rust-lang.org/std/primitive.u64.html#method.checked_shl)
- [CWE-1335: Incorrect Bitwise Shift of Integer](https://cwe.mitre.org/data/definitions/1335.html)
- Related: [`arithmetic_overflow`](arithmetic_overflow.md)
