# `view_panic` — Reachable panic inside a view/getter entrypoint

| | |
| --- | --- |
| **Finding code** | [`SANCT_VIEW_PANIC`](../error-codes.md) |
| **Category** | panic_handling |
| **Severity** | Medium |
| **Source rule** | [`rules/view_panic.rs`](../../tooling/sanctifier-core/src/rules/view_panic.rs) |
| **Glossary** | [View function](../glossary.md#view-function) · [Panic](../glossary.md#panic) |

## What it catches

A view/getter entrypoint — a read-only function whose name reads like an accessor
(`get_*`, `balance`, `price`, …) and that returns a value without mutating state —
that contains a **reachable panic**: an `unwrap()` / `expect()`, an explicit
`panic!`, or raw slice/array indexing that can go out of bounds. Callers (indexers,
dashboards, other contracts) assume reads are total and safe; a getter that traps
instead of returning aborts the whole invocation and can silently break every
consumer that depends on it.

## Vulnerable example

```rust
#[contractimpl]
impl Contract {
    // VULN: getter aborts the read if the price was never set.
    pub fn get_price(env: Env, asset: Symbol) -> i128 {
        env.storage().persistent().get(&asset).unwrap()
    }

    // VULN: raw indexing panics on an out-of-bounds access.
    pub fn get_holder(holders: [u64; 4], idx: usize) -> u64 {
        holders[idx]
    }
}
```

## The fix

Return an Option (or a Result) and let the caller decide how to handle a
missing value, and use checked indexing instead of raw `[idx]`:

```rust
#[contractimpl]
impl Contract {
    // Returns None instead of trapping when the price is unset.
    pub fn get_price(env: Env, asset: Symbol) -> Option<i128> {
        env.storage().persistent().get(&asset)
    }

    // Checked access: no panic on an out-of-bounds index.
    pub fn get_holder(holders: [u64; 4], idx: usize) -> Option<u64> {
        holders.get(idx).copied()
    }
}
```

## How Sanctifier detects it

The rule identifies view/getter entrypoints heuristically (accessor-style names
that return a value and take no mutable/state-writing action) and then walks their
bodies for panic-inducing constructs: `unwrap`/`expect`, `panic!`/`unreachable!`,
and raw indexing expressions. Mutating entrypoints are deliberately excluded — a
panic there is covered by `panic_detection` and
`sanct_unwrap`. `#[cfg(test)]` modules and lines carrying a
`sanctifier:ignore[SANCT_VIEW_PANIC]` justification are skipped.

Limitations: the view/getter classification is name- and shape-based, so a
getter with an unconventional name may be missed, and a helper that provably cannot
panic (e.g. indexing a fixed-size array with a constant) may be a false positive.

## References

- [Soroban docs — Errors and panics](https://soroban.stellar.org/docs/fundamentals/errors)
- [CWE-248: Uncaught Exception](https://cwe.mitre.org/data/definitions/248.html)
- Related: `panic_detection`, `sanct_unwrap`, `unhandled_result`
