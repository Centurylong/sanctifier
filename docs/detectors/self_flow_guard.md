# `self_flow_guard` — Endpoint equality guard

| | |
| --- | --- |
| **Finding code** | [`SANCT_SELF_FLOW_GUARD`](../error-codes.md) |
| **Category** | logic |
| **Severity** | Medium |
| **Source rule** | [`rules/self_flow_guard.rs`](../../tooling/sanctifier-core/src/rules/self_flow_guard.rs) |

## What it catches

The rule complements `edge_amount` by checking endpoint naming shapes that the
older rule does not cover: referral pairs, transfer aliases such as
`sender`/`recipient`, and `transfer_from`-style functions using `from`/`to`.
Ordinary `transfer(from, to, ...)` remains owned by `edge_amount`, so the
default registry does not emit two findings for the same canonical shape.

A finding is emitted when a matching pair is present but the function body
contains no explicit `==` or `!=` comparison between the two endpoints.

## Recognized guards

The detector accepts direct equality/inequality expressions and the standard
`assert_eq!`, `assert_ne!`, `debug_assert_eq!`, and `debug_assert_ne!` macros.
References, parentheses, grouping, and simple `.clone()` wrappers are
normalized for the comparison.

## Scope and limitations

This is a focused syntactic check, not interprocedural data-flow analysis.
Validation delegated entirely to a helper may not be visible to the rule.
Where a shared helper is authoritative, document or suppress the finding
according to the project review policy.

Related detector: [`edge_amount`](edge_amount.md).