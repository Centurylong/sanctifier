# `auth_on_caller` — Authorization on the caller instead of the resource owner

| | |
| --- | --- |
| **Finding code** | [`SANCT_AUTH_ON_CALLER`](../error-codes.md) |
| **Category** | authorization |
| **Severity** | Critical |
| **Source rule** | [`rules/auth_on_caller.rs`](../../tooling/sanctifier-core/src/rules/auth_on_caller.rs) |

## What it catches

An entrypoint calls `require_auth()` on one address parameter, then writes
storage keyed by a **different** address parameter.

The authorization is real — somebody genuinely signed — it is just attached to
the wrong principal. The caller has consented to a change to *somebody else's*
balance, and the contract carries it out on their behalf. This is the classic
[confused deputy](https://en.wikipedia.org/wiki/Confused_deputy_problem): the
contract holds authority over everyone's state and is tricked into exercising it
for a party who never consented.

It is easy to miss in review precisely because the function *looks* authorized.
A detector that only asked "does this entrypoint call `require_auth`?" — which
is what [`auth_gap`](auth_gap.md) asks — sees nothing wrong here.

## Vulnerable example

```rust
#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, caller: Address, owner: Address, amount: i128) {
        caller.require_auth();                       // caller signs...
        let balance = Self::balance_of(&env, &owner) - amount;
        env.storage().persistent().set(&owner, &balance);  // ...owner pays
    }
}
```

Anyone can pass their own address as `caller` and any victim's address as
`owner`, and drain the victim.

## Correct form

```rust
pub fn withdraw(env: Env, owner: Address, amount: i128) {
    owner.require_auth();                            // the payer signs
    let balance = Self::balance_of(&env, &owner) - amount;
    env.storage().persistent().set(&owner, &balance);
}
```

If a third party legitimately acts on the owner's behalf, the owner's consent
still has to exist somewhere — as a recorded allowance the entrypoint checks and
decrements, in the SEP-41 sense. Signing by the spender alone is never enough.

## How it works

The rule is **structural, not name-based**. It does not look for a parameter
called `caller`; a contract whose parameters are named `a` and `b` is analysed
the same way. For each public function it:

1. Collects the parameters whose type mentions `Address`.
2. Records which of them are authorized — receivers of `require_auth()` or
   `require_auth_for_args()`.
3. Records which of them appear in the **key** of a storage write, where the
   receiver chain passes through `storage()`. Those are the resource owners.
4. Reports an owner that is written but not authorized.

## When it does not fire

- **The owner authorizes.** `owner.require_auth()` with a write keyed by
  `owner` is the correct form and is silent, which is the second acceptance
  criterion for the detector.
- **Every address authorizes.** If both `from` and `to` sign, neither is a
  confused deputy.
- **Only one address parameter.** There is no other owner to confuse it with.
- **No authorization at all.** That is [`auth_gap`](auth_gap.md)'s finding, not
  this one. This rule is specifically about auth that is *present but
  misplaced*, so it stays quiet rather than double-reporting.

## Known limits

The owner is inferred from the storage key, so a contract that writes through a
helper (`Self::save(&env, &owner, ..)`) rather than calling `storage().set()`
inline is not seen — the rule is intraprocedural. It also does not model
allowances: an entrypoint that correctly checks a recorded allowance and then
writes the owner's balance is still reported, because the allowance check is not
distinguishable from any other read at this level. Both are false-negative and
false-positive respectively, and both are the usual trade for keeping a
source-level rule fast and dependency-free.
