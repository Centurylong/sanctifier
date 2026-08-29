# `auth_replay` — Custom-account auth without a nonce or expiry

| | |
| --- | --- |
| **Finding code** | [`SANCT_AUTH_REPLAY`](../error-codes.md) |
| **Category** | authentication |
| **Severity** | High |
| **Source rule** | [`rules/auth_replay.rs`](../../tooling/sanctifier-core/src/rules/auth_replay.rs) |

## What it catches

A custom account's `__check_auth` verifies a signature against the payload it
was given, but never reads or increments a nonce and never checks an
expiry/ledger sequence.

Unlike `Address::require_auth()` on a normal account — which the protocol
itself protects against replay — a **custom account**'s `__check_auth` *is*
the entire authentication story. There is nothing underneath it. A signed
payload that is checked only against the public key, with nothing that binds
it to a point in time or consumes a one-time value, remains valid forever.
Anyone who observes the signature once — in a public mempool, an indexer, a
block explorer — can resubmit it indefinitely.

## Vulnerable example

```rust
#[contractimpl]
impl VulnerableAccount {
    pub fn __check_auth(
        env: Env,
        sig_payload: Hash<32>,
        sigs: Vec<Signature>,
        _ctx: Vec<Context>,
    ) -> Result<(), Error> {
        verify(&env, &sig_payload, &sigs)?; // no nonce/expiry consumed
        Ok(())
    }
}
```

## Correct form

Either consume a nonce:

```rust
pub fn __check_auth(env: Env, sig_payload: Hash<32>, sigs: Vec<Signature>, _ctx: Vec<Context>) -> Result<(), Error> {
    verify(&env, &sig_payload, &sigs)?;
    let stored_nonce: u64 = env.storage().persistent().get(&DataKey::Nonce).unwrap_or(0);
    env.storage().persistent().set(&DataKey::Nonce, &(stored_nonce + 1));
    Ok(())
}
```

or bind the payload to an expiry checked against the ledger:

```rust
pub fn __check_auth(env: Env, sig_payload: Hash<32>, sigs: Vec<Signature>, _ctx: Vec<Context>) -> Result<(), Error> {
    verify(&env, &sig_payload, &sigs)?;
    if env.ledger().sequence() > expiration_ledger {
        panic!("signature expired");
    }
    Ok(())
}
```

Either is sufficient on its own; the finding only requires that *something*
nonce- or expiry-shaped is present.

## How it works

1. Finds every `fn`/impl-fn literally named `__check_auth` — the fixed,
   single entry point of `CustomAccountInterface`.
2. Walks its body for any identifier *or* string/symbol literal whose
   lowercased text contains `nonce`, `expir`, `deadline`, `timestamp`, or
   `sequence`. Storage keys are as often a string/symbol literal
   (`symbol_short!("nonce")`) as a bare Rust identifier, so both are checked.
3. Reports `SANCT_AUTH_REPLAY` if neither kind of guard word appears anywhere
   in the function.

This is deliberately a coarse, structural heuristic rather than a data-flow
proof: it does not verify that the nonce is actually checked-and-incremented
correctly, or that the expiry is actually enforced before returning `Ok`. It
only needs to distinguish "nothing replay-shaped is here at all" from "the
author was thinking about replay" — real implementations always name the
field they're checking, so a false negative here would require deliberately
obfuscating a working guard.

## When it does not fire

- **A nonce is read, compared, or incremented anywhere in the function.**
- **An expiry, deadline, timestamp, or ledger sequence is checked anywhere in
  the function**, regardless of exact comparison logic.
- **The function is not named `__check_auth`.** A regular contract entrypoint
  calling `require_auth()` on an `Address` is [`auth_gap`](auth_gap.md)'s or
  [`auth_on_caller`](auth_on_caller.md)'s concern, not this one's — this
  detector only looks at the custom-account authentication entry point
  itself.

## Known limits

The rule does not confirm the nonce/expiry guard is *wired correctly* — a
`__check_auth` that reads a `nonce` field but never stores an updated value,
or names a local variable `expiry_unused` without ever comparing it, would
still pass. It also cannot see through a helper function that does the actual
nonce/expiry check (`fn verify_and_consume_nonce(..)`) if that helper's own
name and body don't mention any guard word — the rule is intraprocedural.
Both are the usual trade for a fast, dependency-free, source-level check.
