# `unbounded_return` — Unbounded Vec or Map returned to caller

| **Code** | [`SANCT_UNBOUNDED_RETURN`](../error-codes.md) |
| --- | --- |
| **Category** | scalability |
| **Severity** | Medium |
| **Source rule** | [`rules/unbounded_return.rs`](../../tooling/sanctifier-core/src/rules/unbounded_return.rs) |

## What it catches

Flags public `#[contractimpl]` entrypoints that return an unbounded collection like a `Vec` or `Map` without providing pagination arguments (like `limit`, `start`, or `cursor`).

Returning a storage-backed array that grows with user activity will eventually hit return-size and memory gas limits, rendering the function uncallable and effectively DoS'ing readers.

## Vulnerable example

```rust
#[contractimpl]
impl Contract {
    pub fn get_all_users(env: Env) -> Vec<Address> {
        let mut users = Vec::new(&env);
        // ... fetches all users from storage ...
        users
    }
}
```

## The fix

Require pagination parameters (`start`, `limit`, or `cursor`) so the caller can fetch the data in safe chunks.

```rust
#[contractimpl]
impl Contract {
    pub fn get_users(env: Env, start: u32, limit: u32) -> Vec<Address> {
        let mut users = Vec::new(&env);
        // ... fetches only a chunk of users from storage ...
        users
    }
}
```

## How Sanctifier detects it

Sanctifier scans the return types of all `pub fn` declarations inside `impl` blocks. If the return type contains `Vec` or `Map`, it checks if the arguments list contains typical pagination parameter names (e.g., `limit`, `offset`, `start`, `page`, `cursor`). If no such parameters are found, it flags the function. Private internal functions are ignored since they are not directly exposed as entrypoints.
