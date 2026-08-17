# Detector-Authoring Guide: Write Your First Rule

A short, hands-on path to shipping your first Sanctifier detector — from an
empty file to a merged PR. For deeper examples (data-flow tracking, multi-step
patterns) once you're past your first rule, see the
[Detector Cookbook](detector-cookbook.md).

## 1. Pick a bug class

Start from something concrete: a pattern you've seen in a real Soroban
contract, or an unclaimed `detector` issue in the tracker. Write one sentence:
*"Flag `<pattern>` unless `<guard>` is present."* That sentence is your rule's
`description()`.

## 2. Scaffold the rule

Create `tooling/sanctifier-core/src/rules/<my_rule>.rs`. The minimal shape:

```rust
use crate::rules::{Rule, RuleViolation, Severity};
use syn::{parse_str, File};
use syn::visit::Visit;

pub struct MyRule;

impl MyRule {
    pub fn new() -> Self { Self }
}
impl Default for MyRule {
    fn default() -> Self { Self::new() }
}

impl Rule for MyRule {
    fn name(&self) -> &str { "my_rule" }
    fn description(&self) -> &str { "Flags <pattern> unless <guard> is present" }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let Ok(file) = parse_str::<File>(source) else { return vec![] };
        let mut visitor = MyVisitor { violations: Vec::new() };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

Sanctifier rules are [`syn`](https://docs.rs/syn)-based AST visitors, not
regex or textual scans — this keeps false positives low and makes guard
tracking (early returns, wrapping `if`s, `assert!`) tractable. Look at
`tooling/sanctifier-core/src/rules/division_by_zero.rs` or
`shift_overflow.rs` for a complete guard-tracking visitor to copy from.

## 3. Register it

Add your module and registration in
`tooling/sanctifier-core/src/rules/mod.rs`:

```rust
pub mod my_rule;
// ...
registry.register(my_rule::MyRule::new());
```

## 4. Add a fixture

Create `tooling/sanctifier-core/tests/fixtures/detectors/my_rule.rs`: a
minimal `#[contract]`/`#[contractimpl]` file with one function that should be
flagged and (ideally) one that shouldn't, so the snapshot documents both the
positive and negative case.

## 5. Snapshot it

Add a test in `tooling/sanctifier-core/tests/detector_snapshots.rs`:

```rust
#[test]
fn snapshot_my_rule() {
    assert_detector_snapshot(
        "my_rule",
        &MyRule::new(),
        include_str!("fixtures/detectors/my_rule.rs"),
    );
}
```

Run `cargo insta test`, review the generated `.snap.new` file with
`cargo insta review` (or `cargo insta accept` once you're confident), and
commit the resulting `.snap` file. See
[`tooling/sanctifier-core/tests/README.md`](../tooling/sanctifier-core/tests/README.md)
for the full testing-framework guide, including how snapshots interact with
`cargo insta` in CI.

## 6. Document it

Every registered detector needs a catalog page, enforced by
`tests/detector_docs_coverage.rs`:

1. `docs/detectors/<my_rule>.md` — follow the structure other pages use
   (What it catches / Vulnerable example / The fix / How Sanctifier detects
   it / References — see [`docs/detectors/README.md`](detectors/README.md#page-anatomy)).
2. A row in [`docs/detectors/README.md`](detectors/README.md)'s table.

Skip either and `cargo test -p sanctifier-core` fails locally and in CI
before you even open a PR.

## 7. Add unit tests and open the PR

Add a `#[cfg(test)] mod tests` block in your rule file covering: the
unguarded case (flagged), each guard shape you support (not flagged), and one
deliberately unrelated pattern (not flagged, proves you're not over-matching).
Run the full local check before pushing:

```sh
cargo fmt --all
cargo test -p sanctifier-core
```

Open a PR referencing the issue (`Closes #NNN`) with a short before/after
example in the description.

## Checklist

- [ ] Rule registered in `rules/mod.rs`
- [ ] Fixture under `tests/fixtures/detectors/`
- [ ] Golden snapshot committed
- [ ] Unit tests for the flagged case, each guard, and a negative case
- [ ] `docs/detectors/<name>.md` + README row
- [ ] `cargo fmt --all` and `cargo test -p sanctifier-core` pass locally
