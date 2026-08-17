# `sep41_approval_expiration` — `approve` without expiration handling

| | |
| --- | --- |
| **Finding code** | [`SANCT_SEP41_APPROVAL_NO_EXPIRATION`](../error-codes.md) |
| **Category** | authorization |
| **Severity** | Error (missing parameter) / Warning (parameter unused) |
| **Source rule** | [`rules/sep41_approval_expiration.rs`](../../tooling/sanctifier-core/src/rules/sep41_approval_expiration.rs) |
| **Glossary** | [Allowance](../glossary.md#allowance) · [TTL](../glossary.md#ttl) |

## What it catches

A [SEP-41](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
`approve`-style entrypoint that never handles an expiration ledger for the
allowance it grants — either because the function has no expiration
parameter at all, or because it accepts one but silently ignores it.

SEP-41's `approve` signature is
`approve(from, spender, amount, expiration_ledger: u32)`, and the standard
requires the allowance to stop being spendable once the current ledger
sequence passes `expiration_ledger`. Two ways contracts drift from this:

1. **No parameter at all.** The allowance is granted with no expiry concept,
   so it lives forever unless separately revoked (`approve(.., 0)`). Any
   integration written against the spec's actual semantics — which assumes
   approvals lapse — ends up trusting a stale, forgotten approval far longer
   than the owner intended.
2. **Parameter present but unused.** The caller's requested expiration is
   accepted and then dropped on the floor: it's never persisted alongside the
   allowance, never used to `extend_ttl`, and never checked before a spend.
   The function *looks* spec-compliant (right signature) while being exactly
   as permanent as case 1.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    // Case 1: no expiration_ledger parameter — this allowance never expires.
    pub fn approve(env: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth();
        write_allowance(&env, &from, &spender, amount);
    }
}
```

```rust
#[contractimpl]
impl Token {
    // Case 2: takes expiration_ledger but never reads it — same bug, spec-
    // shaped signature.
    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        write_allowance(&env, &from, &spender, amount);
    }
}
```

## The fix

Add the parameter, and actually use it — store it alongside the allowance and
extend its storage TTL to it (or otherwise enforce it before a spend):

```rust
#[contractimpl]
impl Token {
    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        write_allowance(&env, &from, &spender, amount);
        env.storage().temporary().extend_ttl(
            &allowance_key(&from, &spender),
            0,
            expiration_ledger,
        );
    }
}
```

## How Sanctifier detects it

The rule finds every `#[contractimpl]` function whose name contains
`approve` and inspects its parameter list for one whose identifier contains
`expir` (matches `expiration_ledger`, `expiry`, ...). If no such parameter
exists, the function is flagged directly. If one does exist, the rule walks
the function body for any expression path referencing that identifier; a
parameter that's never referenced anywhere in the body is flagged as unused.

**Limitations:** it is a name- and reference-based heuristic — a contract
that renames the parameter to something without "expir" in it produces a
false negative for the missing-parameter case being mistaken for present.
Conversely, merely *referencing* the identifier (e.g. logging it) without
actually enforcing it (storing it or checking it against the current ledger)
satisfies the "used" check and won't be flagged — this rule catches
completely-dropped parameters, not partially-wrong enforcement.

## References

- [SEP-0041: Token Interface — `approve`](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
- [Stellar docs — Storage TTL and expiration](https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival)
- Related: [`sep41_allowance_decrement`](sep41_allowance_decrement.md), [`missing_ttl`](missing_ttl.md)
