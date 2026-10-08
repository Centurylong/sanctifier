# `weak_random` — Predictable ledger metadata used as randomness

| | |
| --- | --- |
| **Finding code** | [`SANCT_WEAK_RANDOM`](../error-codes.md) |
| **Category** | cryptography |
| **Severity** | High |
| **Source rule** | [`rules/weak_random.rs`](../../tooling/sanctifier-core/src/rules/weak_random.rs) |
| **Related** | [`ledger_seconds`](ledger_seconds.md) |

## What it catches

Soroban ledger timestamp and sequence are public, predictable metadata. They are
valid inputs for deadlines and ordering, but an attacker can anticipate them.
Using either value as entropy for a lottery, winner, participant, or collection
index makes the outcome predictable and potentially gameable.

## Vulnerable example

```rust
pub fn choose_winner(env: Env, players: Vec<Address>) -> Address {
    let idx = (env.ledger().timestamp() % players.len() as u64) as u32;
    players.get(idx).unwrap()
}
```

## The fix

Use a source that is unpredictable before participants commit to the action.
A commit-reveal scheme can work when its protocol is designed carefully; a
VRF/oracle can provide verifiable external randomness.

```rust
pub fn choose_winner(players: Vec<Address>, reveal: BytesN<32>) -> Address {
    let idx = hash_to_index(reveal) % players.len();
    players.get(idx).unwrap()
}
```

## How Sanctifier detects it

The rule tracks values derived from `env.ledger().timestamp()` or
`env.ledger().sequence()` through local bindings. It reports a finding when a
modulo-reduced tainted value is used as an index / `get`-style selector, or when
a tainted value is passed to an explicitly selection-named call.

Ordinary time logic such as `timestamp() + 60` is intentionally not reported.

**Limitations:** this is local intra-function data-flow. It does not prove
cryptographic unpredictability across helper-function, storage, or cross-contract
boundaries, and it may miss custom selection APIs whose names do not communicate
selection semantics.

## References

- [Stellar developer documentation](https://developers.stellar.org/)
- [CWE-330: Use of Insufficiently Random Values](https://cwe.mitre.org/data/definitions/330.html)
- [CWE-338: Use of Cryptographically Weak PRNG](https://cwe.mitre.org/data/definitions/338.html)
