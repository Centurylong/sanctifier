# `vesting_schedule` — Schedule duration with an unvalidated `end <= start`

| | |
| --- | --- |
| **Finding code** | [`SANCT_VESTING_RANGE`](../error-codes.md) |
| **Category** | logic |
| **Severity** | Warning |
| **Source rule** | [`rules/vesting_schedule.rs`](../../tooling/sanctifier-core/src/rules/vesting_schedule.rs) |
| **Glossary** | [Underflow](../glossary.md#underflow) |

## What it catches

Linear-vesting and other ledger/time-schedule math divides the vested amount by the
span `end - start`. If a caller can supply `end <= start` — a bad request, a
migrated record, or simple integer confusion — that subtraction either:

- **panics** (unsigned underflow, aborting the invocation), or
- **silently yields a nonsense duration** (zero or, on a signed type, negative),
  releasing the full amount immediately or never releasing it at all.

The detector flags `end - start` wherever the left operand's identifier name looks
like an "end" bound (`end`, `end_ledger`, `end_time`, `schedule_end`, ...) and the
right operand's looks like a "start" bound, unless the function already proves
`end > start` first via:

- a sibling early-exit guard: `if end <= start { return ...; }` / `if start >= end { panic!(...); }`,
- a wrapping guard: `if end > start { ... } else { ... }`, or
- a bare `assert!(end > start, ...)` statement.

## Vulnerable example

```rust
#[contractimpl]
impl VestingContract {
    // VULN: no check that end_ledger > start_ledger. A caller passing
    // end_ledger <= start_ledger triggers an unsigned-subtraction panic, or
    // (on a signed duration type) a negative/zero-length schedule.
    pub fn schedule(env: Env, start_ledger: u32, end_ledger: u32, amount: i128) -> i128 {
        let duration = end_ledger - start_ledger;
        amount / (duration as i128)
    }
}
```

## The fix

Validate the range before using it — either an explicit early-return guard or an
`assert!`:

```rust
#[contractimpl]
impl VestingContract {
    pub fn schedule(env: Env, start_ledger: u32, end_ledger: u32, amount: i128) -> i128 {
        if end_ledger <= start_ledger {
            return Err(Error::InvalidSchedule).unwrap();
        }
        let duration = end_ledger - start_ledger;
        amount / (duration as i128)
    }
}
```

## How Sanctifier detects it

The rule walks each function tracking `(start, end)` identifier pairs proven
`end > start` in the current lexical scope — via a diverging sibling `if`, a
wrapping `if`/`else`, or a bare `assert!` — the same guard-tracking approach the
`division_by_zero` detector uses for zero-checks. Any `end - start` subtraction
whose pair isn't in that proven set is reported.

**Limitations:** it matches bound identifiers by *name* (containing `start`/`end`),
so an unconventionally-named pair (e.g. `lo`/`hi`) is a false negative; and it
reasons about a single function, so a guard enforced by a caller or a shared
validation helper is a false positive — add a local `assert!` or early-return guard,
or use `.checked_sub()` and handle the `None` case explicitly.

## References

- [CWE-191: Integer Underflow](https://cwe.mitre.org/data/definitions/191.html)
- [`u32::checked_sub`](https://doc.rust-lang.org/std/primitive.u32.html#method.checked_sub)
- Related: [`division_by_zero`](division_by_zero.md), [`ledger_seconds`](ledger_seconds.md)
