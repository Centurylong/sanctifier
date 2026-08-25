# `unbounded_input_length` — Caller-sized input with no length cap

| | |
| --- | --- |
| **Finding code** | [`SANCT_UNBOUNDED_INPUT`](../error-codes.md) |
| **Category** | denial_of_service |
| **Severity** | Medium |
| **Source rule** | [`rules/unbounded_input_length.rs`](../../tooling/sanctifier-core/src/rules/unbounded_input_length.rs) |

## What it catches

A public entrypoint accepts a `Bytes`, `Vec`, `Map` or `String` argument and
never consults its length.

The caller chooses how big that argument is. Without a cap, an oversized input
exhausts the [resource budget](../glossary.md#resource-metering--budget), and if
the value is persisted it also bloats state permanently at a size the attacker
picked. Neither needs a clever payload — just a large one.

## Relationship to [`arg_dos`](arg_dos.md)

These are complementary, not duplicates, and the split is deliberate:

| | `arg_dos` | `unbounded_input_length` |
| --- | --- | --- |
| Fires when | the argument is **iterated** without a cap | the length is **never checked at all** |
| Types | `Vec`, `Map` | `Bytes`, `Vec`, `Map`, `String` |
| Misses | args that are stored/hashed but never looped | args that are iterated *and* capped elsewhere |

The gap this closes is the `Bytes` blob that is hashed and written to storage
without ever being looped over. `arg_dos` never sees it, because there is no
iteration — but the caller still chose the size.

## Vulnerable example

```rust
pub fn submit_blob(env: Env, blob: Bytes) {
    let digest = env.crypto().sha256(&blob);
    env.storage().persistent().set(&digest, &blob);  // caller picked the size
}
```

## Correct form

```rust
const MAX_BLOB: u32 = 1024;

pub fn submit_blob(env: Env, blob: Bytes) -> Result<(), Error> {
    if blob.len() > MAX_BLOB {
        return Err(Error::InputTooLarge);
    }
    let digest = env.crypto().sha256(&blob);
    env.storage().persistent().set(&digest, &blob);
    Ok(())
}
```

## When it does not fire

- **`.len()` is consulted anywhere in the body.** The check is deliberately
  generous: the rule's job is to find entrypoints where the length is never
  looked at, not to prove the comparison is the right one. Being strict about
  the shape of the bound would fire on every hand-rolled check and make the
  detector unusable.
- **`BytesN<N>`.** Fixed-width by construction — the bound is carried in the
  type, so there is nothing for the caller to inflate.
- **The argument is never used.** It cannot exhaust anything; that is dead
  weight, not a denial-of-service vector.
- **Non-public functions.** The caller does not reach them directly.

## Known limits

The `.len()` check is a heuristic for "the author thought about size", so a
function that reads the length and ignores it is a false negative. Conversely, a
function whose bound is enforced by a helper (`Self::validate(&blob)?`) is
reported, because the rule is intraprocedural. Severity is Medium rather than
High for exactly this reason: it is a prompt to justify the input size, not
proof of an exploitable path.
