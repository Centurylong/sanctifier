# `sep41_allowance_decrement` — `transfer_from` never decrements the allowance

| | |
| --- | --- |
| **Finding code** | [`SANCT_SEP41_ALLOWANCE_NOT_DECREMENTED`](../error-codes.md) |
| **Category** | authorization |
| **Severity** | Error |
| **Source rule** | [`rules/sep41_allowance_decrement.rs`](../../tooling/sanctifier-core/src/rules/sep41_allowance_decrement.rs) |
| **Glossary** | [Allowance](../glossary.md#allowance) · [TOCTOU](../glossary.md#toctou) |

## What it catches

A [SEP-41](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
`transfer_from`-style entrypoint that reads a caller's allowance — typically
to bound the transfer amount — but never writes back a decremented value
before returning.

SEP-41 requires `transfer_from` to reduce the spender's allowance by exactly
the amount transferred. A contract that checks `allowance >= amount` but
forgets the write-back leaves the stored allowance untouched: the *same*
approval can be spent again on the very next call, and the one after that.
Repeated `transfer_from` calls from the same spender then drain far more than
the owner ever authorized — the allowance amount becomes a per-call limit
instead of a lifetime budget, which silently breaks every downstream
integration (DEXs, payment routers, subscription billers) written against the
SEP-41 spec's actual semantics.

## Vulnerable example

```rust
#[contractimpl]
impl Token {
    // Checks the allowance to bound the transfer, but never persists the
    // reduced value — `spender` can call this with the same `amount` as many
    // times as `from`'s balance allows, not just once.
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            panic!("insufficient allowance");
        }

        move_balance(&env, &from, &to, amount);
    }
}
```

## The fix

Persist `allowance - amount` before (or as part of) completing the transfer,
so a repeat call sees the reduced balance:

```rust
#[contractimpl]
impl Token {
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            panic!("insufficient allowance");
        }

        write_allowance(&env, &from, &spender, allowance - amount);
        move_balance(&env, &from, &to, amount);
    }
}
```

## How Sanctifier detects it

The rule finds every `#[contractimpl]` function whose name contains
`transfer_from` (case-insensitive, also matches `transferFrom`) and inspects
its body for two facts: whether it **reads** an allowance (a call whose name
matches `read_allowance` / `get_allowance` / `check_allowance`, or a generic
`.get(..)` / `.read...(..)` call whose name mentions "allowance"), and
whether it **writes** one back (a call matching `write_allowance` /
`set_allowance` / `spend_allowance` / `decrease_allowance` /
`update_allowance`, or a `.set(..)` storage call whose key argument's source
text mentions "allowance"). A function that reads without ever writing is
flagged.

**Limitations:** it is a name-based heuristic over a single function body — a
contract that decrements the allowance through a helper whose name doesn't
match any of the write hints, or that does the write in a *different*
function than the one named `transfer_from`, will be a false negative. A
contract that reads a differently-named value that merely mentions
"allowance" without being the actual approval balance could produce a false
positive; review the flagged function before assuming the fix applies.

## References

- [SEP-0041: Token Interface](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
- [CWE-841: Improper Enforcement of Behavioral Workflow](https://cwe.mitre.org/data/definitions/841.html)
- Related: [`allowance_race`](allowance_race.md), [`sep41_approval_expiration`](sep41_approval_expiration.md)
