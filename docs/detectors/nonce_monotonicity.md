# `nonce_monotonicity`

| Field | Value |
| --- | --- |
| Finding code | `SANCT_NONCE_MONOTONICITY` |
| Category | authentication |
| Default severity | Warning |
| Source rule | `tooling/sanctifier-core/src/rules/nonce_monotonicity.rs` |

## What it catches

Nonce validation that accepts a previously used value or allows the caller to
skip arbitrarily far ahead weakens replay protection. A contract should accept
only the immediate successor to the stored nonce and then persist that accepted
value atomically.

The detector inspects functions with a nonce-named argument. It reports ordered
or direct nonce-to-nonce comparisons that do not express an exact `stored + 1`
relationship, and it reports caller-provided nonce values written to storage
without such a strict comparison.

## Vulnerable example

```rust
pub fn execute(env: Env, provided_nonce: u64) {
    let stored_nonce: u64 = env
        .storage()
        .instance()
        .get(&"nonce")
        .unwrap_or(0);

    // Rejects reuse, but a caller can jump from 4 to 4_000.
    if provided_nonce <= stored_nonce {
        panic!("nonce already used");
    }

    env.storage().instance().set(&"nonce", &provided_nonce);
}
```

Allowing gaps can invalidate pending operations that expected the skipped
nonces. Direct equality checks can permit reuse depending on which branch
accepts the request.

## The fix

```rust
pub fn execute(env: Env, provided_nonce: u64) {
    let stored_nonce: u64 = env
        .storage()
        .instance()
        .get(&"nonce")
        .unwrap_or(0);

    if provided_nonce != stored_nonce + 1 {
        panic!("nonce must increase by exactly one");
    }

    env.storage().instance().set(&"nonce", &provided_nonce);
}
```

Use checked arithmetic when the nonce type could reach its maximum value, and
perform the validation and update in the same contract invocation.

## How Sanctifier detects it

The rule uses the parsed Rust syntax tree rather than text matching. It:

1. selects functions with a nonce-named input;
2. examines comparisons involving nonce-shaped expressions;
3. recognizes `==` or `!=` against an exact `nonce + 1` expression as strict;
4. reports other comparison operators and unvalidated nonce storage writes.

This is a focused heuristic, not whole-program symbolic execution. Helper
functions that hide the comparison or custom successor functions may require
manual review.

## References

- [CWE-294: Authentication Bypass by Capture-replay](https://cwe.mitre.org/data/definitions/294.html)
- [Stellar smart-contract security guidance](https://developers.stellar.org/docs/build/smart-contracts/security)
- [`auth_replay`](auth_replay.md) — missing nonce or expiry protection in custom accounts
