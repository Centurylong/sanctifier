# Case Study: Admin Takeover via Missing `require_auth`

This case study reproduces one of the most common — and most damaging — bug
classes in Soroban audits: a privileged entrypoint that mutates state **without
authenticating the caller**. It walks through catching the bug with Sanctifier,
applying the one-line fix, and rescanning to confirm the finding is gone.

- **Bug class:** broken access control (unauthenticated admin / mint)
- **Finding code:** [`S001` — Authentication Gap](../error-codes.md)
- **Repro contract:** [`contracts/case-studies/admin-takeover/`](../../contracts/case-studies/admin-takeover/)
  ([`vulnerable.rs`](../../contracts/case-studies/admin-takeover/vulnerable.rs) ·
  [`fixed.rs`](../../contracts/case-studies/admin-takeover/fixed.rs))

## Background: why this keeps happening on Soroban

On Stellar/Soroban a contract never implicitly knows "who called me." Unlike an
EVM `msg.sender`, the invoker's identity is only established when the contract
explicitly calls [`Address::require_auth`](https://developers.stellar.org/docs/build/guides/auth).
If a state-mutating function skips that call, **any account can invoke it**.

For an administrative function such as `set_admin` or `mint`, that is a total
compromise: an attacker reassigns the admin to an address they control, or mints
themselves an unlimited balance. Variants of this — "unprotected initializer",
"unauthenticated mint", "admin takeover" — recur across published Soroban and
smart-contract audits (e.g. OpenZeppelin's Soroban reviews and the Stellar
security guidance on authorization), which is why it is Sanctifier's flagship
`S001` detector.

## The vulnerable contract

A trimmed SEP-41-style token. `set_admin` and `mint` write to storage with no
authorization check:

```rust
// contracts/case-studies/admin-takeover/vulnerable.rs
pub fn set_admin(env: Env, new_admin: Address) {
    // VULNERABLE: no admin.require_auth() — anyone can seize control.
    env.storage().instance().set(&DataKey::Admin, &new_admin);
}

pub fn mint(env: Env, to: Address, amount: i128) {
    // VULNERABLE: no authorization — anyone can mint to any account.
    let current: i128 = env.storage().persistent()
        .get(&DataKey::Balance(to.clone())).unwrap_or(0);
    env.storage().persistent()
        .set(&DataKey::Balance(to), &(current + amount));
}
```

### Exploit sketch

1. The legitimate deployer calls `initialize(honest_admin)`.
2. The attacker calls `set_admin(attacker)` directly. No signature is required,
   so the transaction succeeds and the attacker is now admin.
3. The attacker calls `mint(attacker, 1_000_000_000)` and drains value.

No key was stolen and no cryptography was broken — the contract simply never
asked who was calling.

## Catching it with Sanctifier

```console
$ sanctifier analyze contracts/case-studies/admin-takeover/vulnerable.rs

⚠️ Found potential Authentication Gaps!
   -> [S001] Function: initialize
   -> [S001] Function: set_admin
```

Sanctifier flags `set_admin` as an `S001` Authentication Gap: a state-mutating
entrypoint with no `require_auth`. (`initialize` is also listed — a genuine
concern addressed separately with a one-time init guard; see *Remaining
findings* below.)

## The fix

Load the stored admin and require its authorization before mutating state:

```rust
// contracts/case-studies/admin-takeover/fixed.rs
pub fn set_admin(env: Env, new_admin: Address) {
    if let Some(admin) = env.storage().instance().get::<_, Address>(&DataKey::Admin) {
        admin.require_auth();                       // <-- the fix
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }
}

pub fn mint(env: Env, to: Address, amount: i128) {
    if let Some(admin) = env.storage().instance().get::<_, Address>(&DataKey::Admin) {
        admin.require_auth();                       // <-- the fix
        let current: i128 = env.storage().persistent()
            .get(&DataKey::Balance(to.clone())).unwrap_or(0);
        env.storage().persistent()
            .set(&DataKey::Balance(to), &(current + amount));
    }
}
```

Now the host rejects any invocation whose authorization tree does not include a
signature from the current admin.

## Rescanning the fixed contract

```console
$ sanctifier analyze contracts/case-studies/admin-takeover/fixed.rs

⚠️ Found potential Authentication Gaps!
   -> [S001] Function: initialize
✅ No explicit Panics/Unwraps found.
```

`set_admin` (and `mint`) no longer appear under `S001` — the takeover hole is
closed, and the fix introduced no new panics or unwraps.

## Remaining findings (and why they are separate)

- **`S001` on `initialize`** is expected: a first-time initializer has no admin
  to authenticate yet. The standard mitigation is a one-shot guard (store an
  `Initialized` flag and reject a second call), which is an orthogonal hardening
  step, not part of the auth-gap fix demonstrated here.
- Advisory database hits such as `SOB-2024-019` (atomic admin transfer without a
  timelock) are defense-in-depth recommendations layered on top of the now-fixed
  access control, not the vulnerability itself.

## Takeaways

- On Soroban, **every** privileged, state-mutating entrypoint must call
  `require_auth` on the authority it claims to act on behalf of.
- The fix is one line, but the absence of it is catastrophic — exactly the kind
  of omission a static pass catches cheaply.
- Wire `sanctifier analyze` into CI so an `S001` regression fails the build
  before it ever reaches mainnet.
