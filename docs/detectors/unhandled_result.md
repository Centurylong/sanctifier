# `unhandled_result` — Silently dropped `Result`

| | |
| --- | --- |
| **Finding code** | [`S009`](../error-codes.md) |
| **Category** | logic |
| **Severity** | Medium |
| **Source rule** | [`rules/unhandled_result.rs`](../../tooling/sanctifier-core/src/rules/unhandled_result.rs) |
| **Glossary** | [`unwrap` / `expect`](../glossary.md#unwrap--expect) |

## What it catches

A function call that returns a `Result` whose value is neither used, propagated
with `?`, nor explicitly matched — the error path is silently discarded. In a
contract, a dropped `Result` means a failed transfer, a rejected auth, or a
storage error can be **ignored**, letting execution continue as if it succeeded.

## Vulnerable example

```rust
#[contractimpl]
impl Escrow {
    pub fn release(env: Env, to: Address, amount: i128) {
        let client = token::Client::new(&env, &env.storage().instance().get(&TOKEN).unwrap());
        // try_transfer returns a Result; dropping it means a failed payout
        // still marks the escrow as released below.
        client.try_transfer(&env.current_contract_address(), &to, &amount);
        env.storage().instance().set(&RELEASED, &true);
    }
}
```

## The fix

Propagate or handle the error before treating the operation as done:

```rust
#[contractimpl]
impl Escrow {
    pub fn release(env: Env, to: Address, amount: i128) -> Result<(), Error> {
        let client = token::Client::new(&env, &env.storage().instance().get(&TOKEN).unwrap());
        client
            .try_transfer(&env.current_contract_address(), &to, &amount)
            .map_err(|_| Error::TransferFailed)?;
        env.storage().instance().set(&RELEASED, &true);
        Ok(())
    }
}
```

## How Sanctifier detects it

The rule identifies call expressions whose inferred return type is a `Result`
(by naming convention such as `try_*`, and by statement position) used as a bare
statement — i.e. not bound, not `?`-propagated, not matched. Each hit reports the
function and the offending call expression.

**Limitations:** without full type inference it relies on heuristics, so it may
miss `Result`-returning calls that don't follow the convention, and may flag a
call whose `Result` is intentionally ignored (add a justification).

## References

- Rust — [`#[must_use]` and `Result`](https://doc.rust-lang.org/std/result/)
- [CWE-252: Unchecked Return Value](https://cwe.mitre.org/data/definitions/252.html)
- Related: [`panic_detection`](panic_detection.md), [`sanct_unwrap`](sanct_unwrap.md)
