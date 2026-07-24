# `sanct_visibility` — Helper-shaped mutator exposed as an entrypoint

| | |
| --- | --- |
| **Finding code** | [`SANCT_VISIBILITY`](../error-codes.md) |
| **Category** | authentication |
| **Severity** | High |
| **Source rule** | [`rules/sanct_visibility.rs`](../../tooling/sanctifier-core/src/rules/sanct_visibility.rs) |
| **Glossary** | [`require_auth`](../glossary.md#require_auth) · [Authorization](../glossary.md#authorization) |

## What it catches

A public, **helper-shaped** method inside a `#[contractimpl]` block that mutates
contract state without calling `require_auth()` / `require_auth_for_args()`. When
a method looks internal — a leading underscore, or an explicit `helper` /
`internal` in its name — but is still exported, it becomes a callable, unguarded
entrypoint. Anyone can invoke it directly and bypass the checks the "real"
entrypoints perform.

This is the visibility-leak sibling of [`auth_gap`](auth_gap.md): `auth_gap`
flags any state-mutating entrypoint missing an auth guard; `sanct_visibility`
specifically catches methods whose *naming* signals they were never meant to be
public in the first place.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    // Named like a private helper, but `pub` inside #[contractimpl] means it is
    // exported as a contract function. No auth → anyone can set any balance.
    pub fn _set_balance(env: Env, owner: Address, amount: i128) {
        env.storage()
            .persistent()
            .set(&DataKey::Balance(owner), &amount);
    }
}
```

## The fix

Make the intent explicit. Either add an authorization guard if the function is
genuinely a privileged entrypoint, or move the helper out of the
`#[contractimpl]` block so it is not exported:

```rust
#[contractimpl]
impl Token {
    pub fn set_balance(env: Env, admin: Address, owner: Address, amount: i128) {
        admin.require_auth();
        Self::write_balance(&env, &owner, amount);
    }
}

impl Token {
    // Plain impl (no #[contractimpl]) — not exported, callable only internally.
    fn write_balance(env: &Env, owner: &Address, amount: i128) {
        env.storage()
            .persistent()
            .set(&DataKey::Balance(owner.clone()), &amount);
    }
}
```

## How Sanctifier detects it

The rule walks public methods in `#[contractimpl]` blocks, treats a leading
underscore or an `internal`/`helper` naming signal as evidence the method was
meant to be private, and flags it when the body mutates storage without an
authorization call.

**Limitations:** it reasons about naming and a shallow view of the body. A helper
that is genuinely safe to expose (pure reads, or an intentional public utility)
is a false positive — rename it or suppress the finding with a justification.

## References

- Soroban docs — [Authorization](https://soroban.stellar.org/docs/fundamentals-and-concepts/authorization)
- [CWE-749: Exposed Dangerous Method or Function](https://cwe.mitre.org/data/definitions/749.html)
- Related: [`auth_gap`](auth_gap.md)
