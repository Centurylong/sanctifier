# Invariant Library

`sanctifier_core::invariant_library` is a set of reusable, parameterizable
invariant templates for the three properties almost every Soroban contract
needs to state: **conservation**, **monotonicity**, and **access-control**.
Instead of hand-writing a fresh `#[sanctify::invariant(...)]` expression per
contract, pick a template function and drop its rendered string straight into
the attribute — adoption is one line, and the expression stays consistent
(and greppable) across the whole codebase.

These templates render expressions consumed by the existing invariant
scanner, [`sanctifier_core::invariant::scan_invariant_attrs`](../tooling/sanctifier-core/src/invariant.rs);
see that module for how `#[sanctify::invariant(...)]` / `#[invariant(...)]`
attributes are extracted for formal verification.

## Conservation

A quantity's total is unchanged across a state transition — e.g. token supply
across a transfer, pooled reserves across a swap.

```rust
use sanctifier_core::invariant_library::conservation;

#[sanctify::invariant(conservation::total_preserved("total_supply"))]
#[contractimpl]
impl Token {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) { /* ... */ }
}
```

| Function | Checks |
| --- | --- |
| `conservation::total_preserved(field)` | `before.<field> == after.<field>` |
| `conservation::split_sum_preserved(a, b)` | `before.a + before.b == after.a + after.b` |

## Monotonicity

A quantity only ever moves in one direction across state transitions — e.g. a
nonce, a cumulative fee counter, a ledger-sequence-gated unlock.

```rust
use sanctifier_core::invariant_library::monotonicity;

#[sanctify::invariant(monotonicity::non_decreasing("nonce"))]
#[contractimpl]
impl Account {
    pub fn execute(env: Env, nonce: u64) { /* ... */ }
}
```

| Function | Checks |
| --- | --- |
| `monotonicity::non_decreasing(field)` | `after.<field> >= before.<field>` |
| `monotonicity::non_increasing(field)` | `after.<field> <= before.<field>` |

## Access control

A state-mutating transition may only be attributed to an authorized caller.

```rust
use sanctifier_core::invariant_library::access_control;

#[sanctify::invariant(access_control::only_role("admin"))]
#[contractimpl]
impl Vault {
    pub fn set_fee(env: Env, new_fee: i128) { /* ... */ }
}
```

| Function | Checks |
| --- | --- |
| `access_control::only_role(field)` | `caller == before.<field>` |
| `access_control::caller_in_role_set(field)` | `caller ∈ before.<field>` |

## Adding a new template

Templates are plain Rust functions that return the rendered expression
string, grouped into `conservation` / `monotonicity` / `access_control`
submodules of `tooling/sanctifier-core/src/invariant_library.rs`. Each new
template should have a unit test asserting it scans as a valid invariant via
`scan_invariant_attrs` (see the existing `assert_adoptable` helper in that
file's test module) and a row added to the table above.
