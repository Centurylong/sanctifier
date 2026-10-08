# `cyclomatic_complexity` — Excessive branch complexity

| | |
| --- | --- |
| **Finding code** | [`SANCT_CYCLOMATIC_COMPLEXITY`](../error-codes.md) |
| **Category** | maintainability |
| **Severity** | Warning |
| **Source rule** | [`rules/cyclomatic_complexity.rs`](../../tooling/sanctifier-core/src/rules/cyclomatic_complexity.rs) |
| **Related metric** | [`complexity.rs`](../../tooling/sanctifier-core/src/complexity.rs) |

## What it catches

Functions whose cyclomatic complexity is above the configured detector
threshold. Cyclomatic complexity starts at one and increases for branch points
such as `if`, loop constructs, multi-arm `match` expressions, closures, and
short-circuit `&&` / `||` expressions.

The default threshold is 10, matching Sanctifier's existing complexity report.

## Vulnerable example

```rust
pub fn settle(value: i32) -> i32 {
    let mut score = 0;
    if value > 0 { score += 1; }
    // many more independent branches ...
    score
}
```

When branch count pushes the function above the configured threshold, Sanctifier
reports a hotspot and suggests splitting or simplifying the control flow.

## The fix

Prefer smaller helpers with a single responsibility or data-driven control flow
when that makes the branch structure easier to reason about and test. Do not
refactor solely to silence the metric if the split would obscure important
state transitions.

Library users can choose a project-appropriate threshold:

```rust
use sanctifier_core::rules::cyclomatic_complexity::CyclomaticComplexityRule;

let rule = CyclomaticComplexityRule::with_threshold(15);
```

## How Sanctifier detects it

The rule delegates metric calculation to the shared `complexity` module rather
than maintaining a second branch-counting implementation, then emits a finding
for every analyzed function whose metric is strictly greater than the configured
threshold.

**Limitations:** this is a structural maintainability signal, not proof of a
security bug. Generated code and intentionally branch-heavy state machines may
need a higher threshold or targeted refactoring.

## References

- [Cyclomatic complexity](https://en.wikipedia.org/wiki/Cyclomatic_complexity)
- Related: Sanctifier's contract complexity report in `complexity.rs`
