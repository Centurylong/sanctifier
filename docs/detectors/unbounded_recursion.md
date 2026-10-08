# `unbounded_recursion` — Direct recursion without a depth bound

| | |
| --- | --- |
| **Finding code** | [`SANCT_UNBOUNDED_RECURSION`](../error-codes.md) |
| **Category** | resource_limits |
| **Severity** | High |
| **Source rule** | [`rules/unbounded_recursion.rs`](../../tooling/sanctifier-core/src/rules/unbounded_recursion.rs) |

## What it catches

Direct self-recursion in contract source when Sanctifier cannot see an explicit,
monotonic depth bound. Unbounded recursion can exhaust the execution budget or
stack before the contract reaches a useful result.

## Vulnerable example

```rust
pub fn recurse_forever(env: Env, value: u32) {
    if value > 0 {
        Self::recurse_forever(env, value - 1);
    } else {
        Self::recurse_forever(env, value);
    }
}
```

The data argument changes, but there is no explicit recursion-depth contract:
one branch can call itself forever.

## Bounded form

```rust
pub fn walk(env: Env, value: u32, depth: u32) {
    if depth >= 8 {
        return;
    }
    Self::walk(env, value, depth + 1);
}
```

A countdown such as `remaining - 1` guarded at zero is also recognized.

## How it works

The rule is intentionally intraprocedural and conservative:

1. Find a function or impl method that directly calls itself.
2. Identify depth-like parameters such as `depth`, `remaining`, or
   `*_left`.
3. Require a comparison involving that parameter and require recursive calls
   to change it monotonically with a positive add/subtract step (including
   checked/saturating add/sub forms).
4. Emit one finding for the function when direct recursion exists and no such
   explicit bound is visible.

## Known limits

This first focused rule does not construct a call graph, so mutual recursion
(`a -> b -> a`) is outside its scope. It also does not attempt theorem proving
for domain-specific decreasing measures: a function that terminates because a
tree/list argument shrinks but exposes no explicit depth parameter may still be
reported. Prefer an explicit depth budget for contract recursion because that
also documents the metering boundary for callers and reviewers.
