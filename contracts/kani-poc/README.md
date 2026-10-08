# Kani admin write invariant (#341)

This proof of concept now couples a pure, Kani-verifiable admin transition model to the Soroban contract's **admin storage mutation path**. The preexisting `set_admin` entrypoint wrote unconditionally; it no longer does.

## Contract boundary and behavior

- `initialize(env, name, admin: Address)` requires the initial admin's signature, checks one-shot initialization, and persists `admin` and `init` only after successful authorization.
- `set_admin(env, current_admin: Address, new_admin: Address)` loads the persisted admin, rejects a mismatching address, invokes `current_admin.require_auth()`, and writes the new Address **only after** this Host call returns. A failed signature/authentication traps before the storage mutation.
- This changes the former PoC signatures `initialize(env, name)` and `set_admin(env, new_admin: Symbol)`. Callers of this PoC must supply signed Address arguments.

Kani cannot symbolically execute Soroban's Host FFI. Instead, `admin_write_pure` models a storage transition from arbitrary admin/caller IDs, an arbitrary `auth_succeeded` verdict, and an arbitrary replacement ID. The verdict represents whether `Address::require_auth()` would return (true) or trap (false). The production path uses the same `admin_write_allowed` predicate after a real Host check. This proves the **pure authorization-and-write transition**, not the underlying Host signature verification implementation or the full Env runtime.

The passing proof deliberately **does not** assume that callers are admins or authenticated. It checks that either denial condition causes zero writes and no state change, and that a permitted caller reaches the write with the expected new state. Write reachability is tracked with an explicit `wrote` flag so writing an unchanged Address does not hide a security fault.

## Focused verification

With `cargo-kani` installed and the repository's Rust dependencies available:

```sh
cargo kani -p kani-poc-contract --harness verify_admin_write_requires_current_authenticated_admin
cargo kani -p kani-poc-contract --harness verify_old_admin_is_revoked_after_rotation
cargo kani -p kani-poc-contract --harness verify_missing_require_auth_is_caught
cargo test -p kani-poc-contract admin_write_requires_matching_admin_and_host_auth
```

- `verify_admin_write_requires_current_authenticated_admin`: Kani should prove rejection of any unauthenticated or mismatching caller, while preserving the authorized transition.
- `verify_old_admin_is_revoked_after_rotation`: an administrator's valid initial rotation must revoke that former principal immediately, even if a later Host call verifies the former signature.
- `verify_missing_require_auth_is_caught`: expected **negative control**, marked `#[kani::should_panic]`. It gives the buggy variant a matching supplied admin ID but no Host authorization; Kani should find a failing assertion when the buggy version writes anyway. This is not a claim that the buggy function is safe.
- The single Rust unit test checks both denial reasons, one allowed update, and the negative-control write.

### Evidence boundary

The code is authored; without a functioning Kani-enabled Rust environment, no successful compilation, Kani proof result, or real Soroban Host-execution result is claimed. Run the three targeted commands before treating formal-verification acceptance as complete.
