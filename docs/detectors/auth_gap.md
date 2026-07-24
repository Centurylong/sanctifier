# `auth_gap` — Authentication gap

| | |
| --- | --- |
| **Finding code** | [`S001`](../error-codes.md) |
| **Category** | authentication |
| **Severity** | Critical |
| **Source rule** | [`rules/auth_gap.rs`](../../tooling/sanctifier-core/src/rules/auth_gap.rs) |
| **Glossary** | [Authorization gap](../glossary.md#authorization-gap-auth-gap) · [`require_auth`](../glossary.md#require_auth) |

## What it catches

A public contract function mutates persistent or instance state but never calls
[`require_auth`](../glossary.md#require_auth) (or `require_auth_for_args`) on the
address whose assets or permissions it affects. Because Soroban invocations are
public, an unauthenticated state-mutating entrypoint lets **any** caller move
funds, change admins, or overwrite records that should belong to a specific
account. This is the single most common critical bug class in Soroban contracts.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    // Anyone can call this and drain `from` — no proof the caller controls it.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let mut balances = env.storage().persistent();
        let from_bal: i128 = balances.get(&from).unwrap_or(0);
        balances.set(&from, &(from_bal - amount));
        let to_bal: i128 = balances.get(&to).unwrap_or(0);
        balances.set(&to, &(to_bal + amount));
    }
}
```

## The fix

Require the source account to authorize the call before touching its state:

```rust
#[contractimpl]
impl Token {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth(); // caller must prove control of `from`

        let mut balances = env.storage().persistent();
        let from_bal: i128 = balances.get(&from).unwrap_or(0);
        balances.set(&from, &(from_bal - amount));
        let to_bal: i128 = balances.get(&to).unwrap_or(0);
        balances.set(&to, &(to_bal + amount));
    }
}
```

For calls acting on behalf of a privileged role, authenticate the admin address
loaded from storage (`admin.require_auth()`), not a caller-supplied one.

## How Sanctifier detects it

The rule parses each `#[contractimpl]` method with `syn`, checks whether the body
performs a storage mutation (`storage().*.set/update/remove`), and flags the
function when no `require_auth`/`require_auth_for_args` call is present on an
`Address` argument. View-only functions and functions that authenticate are not
reported.

**Limitations:** authentication routed through a helper function that Sanctifier
cannot inline may produce a false positive — suppress with
`// sanctifier-ignore: S001 - <justification>`.

## References

- Soroban docs — [Authorization](https://soroban.stellar.org/docs/fundamentals-and-concepts/authorization)
- [CWE-862: Missing Authorization](https://cwe.mitre.org/data/definitions/862.html)
- Related: [`hardcoded_addr`](hardcoded_addr.md), [`edge_amount`](edge_amount.md)
