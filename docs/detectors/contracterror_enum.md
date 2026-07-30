# `contracterror_enum` — Missing `#[contracterror]` or unstable repr on error enum

| **Code** | [`SANCT_CONTRACTERROR_ENUM`](../error-codes.md) |
| --- | --- |
| **Category** | logic |
| **Severity** | Warning |
| **Source rule** | [`rules/contracterror_enum.rs`](../../tooling/sanctifier-core/src/rules/contracterror_enum.rs) |

## What it catches

Flags public functions returning a `Result` where the error type is an enum missing the `#[contracterror]` attribute or missing a stable `#[repr(...)]` (like `#[repr(u32)]`). Unstable or non-contract error types break client decoding of errors on the Soroban network.

## Vulnerable example

```rust
pub enum Error {
    AuthFailed = 1,
}

#[contractimpl]
impl Token {
    pub fn do_thing(env: Env) -> Result<(), Error> {
        Err(Error::AuthFailed)
    }
}
```

## The fix

Annotate the error enum with `#[contracterror]` and a stable `#[repr(...)]`:

```rust
#[contracterror]
#[repr(u32)]
pub enum Error {
    AuthFailed = 1,
}

#[contractimpl]
impl Token {
    pub fn do_thing(env: Env) -> Result<(), Error> {
        Err(Error::AuthFailed)
    }
}
```

## How Sanctifier detects it

The detector visits all `enum` declarations, tracking which have `#[contracterror]` and `#[repr]`. It then visits all `pub fn` functions returning `Result<T, E>`. If `E` is a local enum that lacks these attributes, it flags the function.
