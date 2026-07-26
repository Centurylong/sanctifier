# `tier_boundary_off_by_one` — Inconsistent boundary comparisons in a tier/rank ladder

| | |
| --- | --- |
| **Finding code** | [`S022`](../error-codes.md) |
| **Category** | logic |
| **Severity** | Info |
| **Source rule** | [`rules/tier_boundary_off_by_one.rs`](../../tooling/sanctifier-core/src/rules/tier_boundary_off_by_one.rs) |

## What it catches

An `if`/`else if` ladder that assigns a tier, rank, or similar bucket based on
where a value falls relative to a set of thresholds — but mixes **strict**
(`<`, `>`) and **inclusive** (`<=`, `>=`) comparisons against the *same*
variable across sibling branches. A well-formed ladder uses one comparison
convention for its entire length, so every possible value belongs to exactly
one branch. Mixing the two is the classic off-by-one signature: the shared
boundary value either matches two branches (the earlier one silently wins) or
matches neither (falling through to an unintended branch).

The rule only looks at contiguous branches that compare the *same* simple
variable against a numeric literal; a ladder branching on different variables,
or a lone `if` with no `else if`, is not a ladder and is left alone.

## Vulnerable example

```rust
#[contractimpl]
impl Loyalty {
    // score == 80 falls into Silver here, but the mix of `<` and `<=` means
    // the boundary was never deliberately decided — a future edit to either
    // branch is likely to silently reintroduce a gap or an overlap.
    pub fn tier_of(env: Env, score: u32) -> Tier {
        if score < 50 {
            Tier::Bronze
        } else if score <= 80 {
            Tier::Silver
        } else {
            Tier::Gold
        }
    }
}
```

## The fix

Pick one comparison operator for the whole ladder and use it consistently, with
thresholds chosen so the boundaries are unambiguous:

```rust
#[contractimpl]
impl Loyalty {
    pub fn tier_of(env: Env, score: u32) -> Tier {
        if score < 50 {
            Tier::Bronze
        } else if score < 80 {
            Tier::Silver
        } else {
            Tier::Gold
        }
    }
}
```

## How Sanctifier detects it

The rule walks each `if`/`else if` chain. For every branch whose condition is a
simple `variable OP literal` (or `literal OP variable`) comparison, it records
the ladder's subject variable, the comparison normalized to variable-first, and
the threshold. Comparisons against a threshold on the left-hand side (`50 >
score`) are normalized the same way as the more common `score < 50` form. Once
two or more branches share a subject variable, adjacent branches are compared:
if one uses `<`/`>` and its neighbor uses the inclusive counterpart (`<=`/`>=`)
in the same direction, it's flagged. A chain is only followed as long as
consecutive branches keep comparing the same variable — once a branch
compares something else (or isn't a simple threshold comparison at all), the
ladder ends there for this rule's purposes, and nested `if`/`else if` chains
inside a branch body are still visited independently.

**Limitations:** this is a syntactic heuristic, not a proof that the mixed
ladder is actually wrong — some codebases intentionally use `<=` on the last
comparison for stylistic reasons without an actual bug. It also only tracks a
single variable and integer-literal thresholds per branch; ladders that
compare derived expressions (`score / 10`) or two different variables are not
analyzed for boundary consistency.

## References

- [CWE-193: Off-by-one Error](https://cwe.mitre.org/data/definitions/193.html)
- Related: [`arithmetic_overflow`](arithmetic_overflow.md), [`balance_equality`](balance_equality.md)
