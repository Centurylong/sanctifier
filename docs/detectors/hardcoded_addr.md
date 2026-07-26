# `hardcoded_addr` — Hardcoded address / secret

| | |
| --- | --- |
| **Finding code** | [`S012`](../error-codes.md) |
| **Category** | code_hygiene |
| **Severity** | High |
| **Source rule** | [`rules/hardcoded_addr.rs`](../../tooling/sanctifier-core/src/rules/hardcoded_addr.rs) |
| **Glossary** | [Hardcoded address](../glossary.md#hardcoded-address) · [Address](../glossary.md#address) |

## What it catches

A Stellar address (`G…`/`C…` strkey) or secret literal baked into source and used
in an authentication or authorization context. A hardcoded admin address can't be
rotated without a redeploy, is copied across forks/testnets, and — for secret
seeds — leaks signing authority to anyone who reads the repo.

## Vulnerable example

```rust
#[contractimpl]
impl Admin {
    pub fn set_fee(env: Env, caller: Address, bps: u32) {
        // Baked-in admin: cannot rotate, and every deployment shares it.
        let admin = Address::from_string(&String::from_str(
            &env,
            "GBADMIN000000000000000000000000000000000000000000000000",
        ));
        caller.require_auth();
        assert_eq!(caller, admin);
        env.storage().instance().set(&FEE, &bps);
    }
}
```

## The fix

Store the admin in contract state, set once at initialization, and rotate through
an authenticated path:

```rust
#[contractimpl]
impl Admin {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn set_fee(env: Env, bps: u32) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&FEE, &bps);
    }
}
```

Never embed secret seeds in contract or tooling source; load them from the
environment or a signer.

## How Sanctifier detects it

The rule scans string literals for Stellar strkey shapes (`G`/`C`-prefixed
base32 of the right length) and secret-like patterns, and reports those used near
auth checks or admin assignments.

**Limitations:** documented example addresses in comments/tests can trip it —
suppress those with a justification.

## References

- Soroban docs — [Authorization](https://soroban.stellar.org/docs/fundamentals-and-concepts/authorization)
- [CWE-798: Use of Hard-coded Credentials](https://cwe.mitre.org/data/definitions/798.html)
- Related: [`auth_gap`](auth_gap.md)
