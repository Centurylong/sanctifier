# `allowance_race` — Allowance overwritten unconditionally (approve TOCTOU)

| | |
| --- | --- |
| **Finding code** | [`SANCT_ALLOWANCE_RACE`](../error-codes.md) |
| **Category** | authorization |
| **Severity** | Medium |
| **Source rule** | [`rules/allowance_race.rs`](../../tooling/sanctifier-core/src/rules/allowance_race.rs) |
| **Glossary** | [Allowance](../glossary.md#allowance) · [Front-running](../glossary.md#front-running) |

## What it catches

An `approve`-style entrypoint that **overwrites** a stored allowance with a
caller-supplied amount, with no delta or compare-and-set semantics. This is the
classic ERC-20 *approve front-running race* (a time-of-check to time-of-use bug):
a spender watching the mempool sees an allowance about to change from `N` to `M`,
spends the old `N` before the change lands, then spends the new `M` as well —
drawing `N + M` against an owner who only ever intended one of the two.

The detector treats a function as allowance-related when its name contains
`approve` or `allowance`, or when it writes to an allowance-keyed storage entry.
The following **safe shapes are not flagged**:

- **Delta semantics** — `increase_allowance` / `decrease_allowance`, where the new
  value is computed from the current one (`current + delta`).
- **Compare-and-set** — the caller passes the allowance it expects to be current
  (an `expected_current`-style parameter) and the write only lands on a match.
- **Read-before-write** — the current allowance is read via storage `.get()`
  before the new value is written.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    // VULN: blindly overwrites the stored allowance from a caller-supplied
    // amount — the approve front-running race.
    pub fn approve(e: Env, owner: Address, spender: Address, amount: i128) {
        owner.require_auth();
        e.storage().persistent().set(&(owner, spender), &amount);
    }
}
```

## The fix

Use delta semantics so the change is relative to the current allowance, or require
the caller to prove the value it expects to replace (compare-and-set):

```rust
#[contractimpl]
impl Token {
    // Delta semantics: the write is computed from the current allowance, so a
    // racing spend cannot double-draw.
    pub fn increase_allowance(e: Env, owner: Address, spender: Address, delta: i128) {
        owner.require_auth();
        let key = (owner, spender);
        let current: i128 = e.storage().persistent().get(&key).unwrap_or(0);
        e.storage().persistent().set(&key, &(current + delta));
    }

    // Compare-and-set: the write only lands if the stored value still matches
    // what the caller expected.
    pub fn approve_checked(
        e: Env,
        owner: Address,
        spender: Address,
        expected_current: i128,
        amount: i128,
    ) {
        owner.require_auth();
        let key = (owner, spender);
        let stored: i128 = e.storage().persistent().get(&key).unwrap_or(0);
        if stored == expected_current {
            e.storage().persistent().set(&key, &amount);
        }
    }
}
```

## How Sanctifier detects it

The rule parses each `#[contractimpl]` entrypoint and, for allowance-related
functions, looks for an unconditional storage `set` (an overwrite) that is **not**
preceded by a read of the current allowance and is **not** shaped as a delta or
compare-and-set. Only storage-backed writes (`persistent()` / `instance()` /
`temporary()`) count, so plain local assignments are ignored.

**Limitations:** it reasons syntactically about a single function. An allowance
guarded by logic in a helper it cannot see, or one using an unconventional naming
scheme, may be a false negative; a differently-named setter that legitimately
performs a full reset may be a false positive — suppress with a justified
`sanctifier:ignore[SANCT_ALLOWANCE_RACE]` in that case.

## References

- [EIP-20: `approve` race condition](https://eips.ethereum.org/EIPS/eip-20#approve) — the canonical write-up and the increase/decrease convention.
- [CWE-367: Time-of-check Time-of-use (TOCTOU) Race Condition](https://cwe.mitre.org/data/definitions/367.html)
- Related: [`auth_gap`](auth_gap.md), [`edge_amount`](edge_amount.md)
