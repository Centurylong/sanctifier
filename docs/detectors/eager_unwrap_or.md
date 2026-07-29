# `eager_unwrap_or` — Eagerly-computed expensive fallback in `unwrap_or()`

| **Code** | [`SANCT_EAGER_UNWRAP_OR`](../error-codes.md) |
| --- | --- |
| **Category** | gas_efficiency |
| **Severity** | Warning |
| **Source rule** | [`rules/eager_unwrap_or.rs`](../../tooling/sanctifier-core/src/rules/eager_unwrap_or.rs) |

## What it catches

Flags usage of `.unwrap_or(value)` where `value` involves an expensive computation, such as a function call, a macro, or a complex expression. `unwrap_or()` eagerly evaluates its argument regardless of whether the primary `Option` or `Result` is successful, which can waste execution gas on the "hit" path.

## Vulnerable example

```rust
let config = env.storage().persistent().get(&Key::Config)
    .unwrap_or(compute_expensive_default_config(&env));
```

In this example, `compute_expensive_default_config` is always evaluated, even if the configuration already exists in storage.

## The fix

Use `.unwrap_or_else(|| ...)` to defer the evaluation until it is genuinely needed:

```rust
let config = env.storage().persistent().get(&Key::Config)
    .unwrap_or_else(|| compute_expensive_default_config(&env));
```

## How Sanctifier detects it

The detector scans the abstract syntax tree for method calls named `unwrap_or`. If the argument passed to `unwrap_or` is a function call, a method call, or a macro invocation, the rule flags it as a potential gas leak. Literal values (like `0` or `false`) are ignored.
