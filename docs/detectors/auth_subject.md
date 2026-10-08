# `auth_subject` — Storage-loaded effect owner differs from the authenticated principal

| Field | Value |
| --- | --- |
| Finding code | `SANCT_AUTH_SUBJECT` |
| Severity | High (`Severity::Error`) |
| Category | authorization |
| Source | [`auth_subject.rs`](../../tooling/sanctifier-core/src/rules/auth_subject.rs) |

## The confused deputy

A caller can authorize an entrypoint while a different account is loaded from
contract storage and debited. Checking only that someone authorized the
invocation is insufficient; the authorization must bind the owner affected
by the change or a properly verified allowance.

```rust
pub fn withdraw(e: Env, from: Address, amount: i128) {
    from.require_auth();
    let victim: Address = e.storage().persistent().get(&0).unwrap();
    debit(&e, &victim, amount); // SANCT_AUTH_SUBJECT
}
```

A directly safe variant authorizes `victim` before the debit, rather than
authorizing the caller. Authorization of the sender in a normal transfer
also remains safe even when the transfer credits a different receiver.

## Detection and limitations

This default-enabled rule tracks `Address` entrypoint parameters, `.clone()`
aliases, addresses loaded via recognized `storage().*.get()` calls, and
storage-handle aliases in public free functions and public methods. It
collects `require_auth` / `require_auth_for_args` subjects and recognizes
direct storage `set` / `remove` / `update` and explicit helper calls named
`debit`, `withdraw`, `debit_balance`, `burn_from`, `spend_from` and
`set_balance`. An effect attributed to a storage-loaded owner is flagged
when no state owner resolves to an authenticated principal. A storage-sourced **debit/withdraw/burn/remove** is also reported when its owner is unauthenticated, even if a different written account belongs to the signer. Ordinary recipient credits remain silent.

Unknown address provenance is not treated as proof of a mismatch.
The check is intraprocedural and conservative: renamed helpers, indirect
calls, paths through other functions, branch-dependent aliases and
custom allowance semantics need manual review. Findings are indications,
not proofs of a runtime exploit. Inspect the helper's behavior before
treating it as a state mutation.

Focused Rust regression cases are authored alongside the rule for
storage-loaded victims, aliases, correct auth, normal transfer,
mixed victim-debit/attacker-credit, recipient credits, and unknown or private flows; they are not a substitute for runtime tests.
