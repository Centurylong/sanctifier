# SEP-41 Formal Spec Template

A reusable, formally verified invariant set for SEP-41 token compliance.

## Overview

This template provides a complete set of invariants capturing SEP-41 token
semantics, implemented as pure arithmetic functions and proven with Kani/SMT.
Any token that adopts this template can answer **"is this a correct SEP-41
token?"** as a verifiable checklist.

Location: `contracts/sep41-token-invariants/`

## Invariants Encoded

| # | Invariant                                      | SEP-41 Requirement                 | Status |
|---|------------------------------------------------|------------------------------------|--------|
| 1 | Supply conservation across `transfer`          | Σ balances == total_supply         | Kani ✅ |
| 2 | Transfer rejects non-positive amount           | Reverts on amount ≤ 0              | Kani ✅ |
| 3 | Transfer rejects insufficient balance          | Reverts when from < amount         | Kani ✅ |
| 4 | Supply conservation across `transfer_from`     | Σ balances unchanged + allowance ↓ | Kani ✅ |
| 5 | Transfer-from rejects insufficient allowance   | Reverts when allowance < amount    | Kani ✅ |
| 6 | Approve sets allowance correctly               | allowance == approved amount       | Kani ✅ |
| 7 | Approve rejects negative amount                | Reverts on amount < 0              | Kani ✅ |
| 8 | Burn reduces balance by amount                 | new = balance - amount             | Kani ✅ |
| 9 | Burn rejects insufficient balance              | Reverts when balance < amount      | Kani ✅ |
| 10 | Burn rejects non-positive amount               | Reverts on amount ≤ 0              | Kani ✅ |
| 11 | Mint increases balance by amount               | new = balance + amount             | Kani ✅ |
| 12 | Mint rejects non-positive amount               | Reverts on amount ≤ 0              | Kani ✅ |

## File Structure

```
contracts/sep41-token-invariants/
├── Cargo.toml              # Package + dependencies (soroban-sdk, sanctify-macros)
├── src/
│   ├── lib.rs              # Soroban contract + #[invariant(...)] attributes
│   ├── pure.rs             # Pure arithmetic functions (no Host dependency)
│   └── kani_proofs.rs      # Kani symbolic proofs (14 harnesses)
```

## How to Use This Template

### 1. Copy the pure functions

Copy `src/pure.rs` into your project. It has **no Soroban dependencies** and
contains all 12 invariant-checking functions. Each function returns `bool` and
follows the "invalid input is a no-op → invariant trivially holds" pattern.

### 2. Annotate your contract

Add `#[invariant(...)]` attributes to your `#[contractimpl]` block:

```rust
use sanctify_macros::invariant;

#[invariant(pure::supply_conserved_after_transfer(0, 0, 0))]
#[invariant(pure::supply_conserved_after_transfer_from(0, 0, 0, 0))]
#[invariant(pure::allowance_is_set_by_approve(0))]
#[contractimpl]
impl MyToken {
    // ...
}
```

### 3. Run the proofs

```sh
# Symbolic verification via Kani
cargo kani --package sep41-token-invariants

# Static analysis via Sanctifier
sanctifier verify
```

### 4. Verify compliance

Once all 12 invariants pass Kani and Sanctifier verification, your token is
formally proven to satisfy core SEP-41 arithmetic semantics.

## Design Principles

### Core Logic Separation

All verified logic lives in `pure.rs` — plain Rust functions with no
`Env` or `Host` types. This avoids the `extern "C"` FFI barrier that
prevents Kani from seeing into Soroban SDK internals.

The contract layer (`lib.rs`) is intentionally **thin**: it marshals
data between storage and pure functions, but all arithmetic decisions
are delegated to the verified layer.

### No-op Semantics for Invalid Inputs

Every invariant function returns `true` for invalid inputs. This
captures the property that a reverted transaction is a no-op —
the state is unchanged, so the invariant trivially holds.

### Compositional Verification

Each invariant is proven independently, allowing token developers to:
- Mix and match invariants appropriate for their use case
- Inherit proven properties without re-verifying
- Add custom invariants alongside the standard set

## References

- [SEP-41: Soroban Token Interface](https://stellar.org/protocol/sep-41)
- [Kani Rust Verifier](https://model-checking.github.io/kani/)
- [Sanctifier Architecture](../../ARCHITECTURE.md)
