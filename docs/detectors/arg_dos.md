# `arg_dos` — Unbounded iteration over an argument

| | |
| --- | --- |
| **Finding code** | [`SANCT_ARG_DOS`](../error-codes.md) |
| **Category** | denial_of_service |
| **Severity** | High |
| **Source rule** | [`rules/arg_dos.rs`](../../tooling/sanctifier-core/src/rules/arg_dos.rs) |
| **Glossary** | [Resource metering / budget](../glossary.md#resource-metering--budget) · [OOG](../glossary.md#oog-out-of-gas) |

## What it catches

A contract entrypoint takes a `Vec<_>` or `Map<_, _>` argument and iterates over
it without a visible length cap. Because the caller controls the argument's
length, they can pass an oversized collection that blows the
[resource budget](../glossary.md#resource-metering--budget) — the call runs
[out of gas](../glossary.md#oog-out-of-gas) and reverts. When such a call is on a
critical path (batch settlement, admin config), this is a denial-of-service.

## Vulnerable example

```rust
#[contractimpl]
impl Payroll {
    // Caller decides how many recipients — an oversized Vec exhausts the budget.
    pub fn pay_all(env: Env, from: Address, payments: Vec<(Address, i128)>) {
        from.require_auth();
        for (to, amount) in payments.iter() {
            transfer(&env, &from, &to, amount);
        }
    }
}
```

## The fix

Cap the length before iterating, and paginate large batches:

```rust
const MAX_BATCH: u32 = 100;

#[contractimpl]
impl Payroll {
    pub fn pay_all(env: Env, from: Address, payments: Vec<(Address, i128)>) -> Result<(), Error> {
        from.require_auth();
        if payments.len() > MAX_BATCH {
            return Err(Error::BatchTooLarge);
        }
        for (to, amount) in payments.iter() {
            transfer(&env, &from, &to, amount);
        }
        Ok(())
    }
}
```

## How Sanctifier detects it

The rule identifies `#[contractimpl]` entrypoints whose parameters include a
`Vec`/`Map`, then checks whether the body iterates that parameter (`.iter()`,
`for … in`) without a preceding comparison of its `.len()` against a bound. Loops
guarded by a length check are not reported.

**Limitations:** a cap enforced in a called helper, or a bound derived
indirectly, may not be recognized — suppress with a justification.

## References

- Soroban docs — [Fees and metering](https://soroban.stellar.org/docs/fundamentals-and-concepts/fees-and-metering)
- [CWE-400: Uncontrolled Resource Consumption](https://cwe.mitre.org/data/definitions/400.html)
- Related: [`ledger_size`](ledger_size.md)
