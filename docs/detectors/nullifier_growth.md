# `nullifier_growth` — Nullifier/commitment set grows without pruning or expiry

| | |
| --- | --- |
| **Finding code** | [`SANCT_NULLIFIER_GROWTH`](../error-codes.md) |
| **Category** | denial_of_service |
| **Severity** | Warning |
| **Source rule** | [`rules/nullifier_growth.rs`](../../tooling/sanctifier-core/src/rules/nullifier_growth.rs) |
| **Glossary** | [Ledger entry](../glossary.md#ledger-entry) · [State bloat](../glossary.md#state-bloat) |

## What it catches

Soroban ZK-verifier contracts (shielded pools, private-transfer contracts, proof
verifiers) prevent double-spends by writing a **nullifier** (or note commitment) into
persistent storage the first time it is spent, then rejecting any future spend that
reuses the same nullifier. That store is intentionally append-only from the
contract's point of view — a nullifier can never be "un-spent" — but nothing about
Soroban makes the *entries* free. Every spend adds one more durable ledger key that
is never reclaimed, so the nullifier set grows without bound as usage grows.

This rule flags a `spend` / `verify` / `nullify` / `redeem` / `claim` / `withdraw`
entrypoint that writes a nullifier- or commitment-shaped key into
`storage().persistent()` (e.g. `env.storage().persistent().set(&nullifier, &true)`)
with **no corresponding**:

- **pruning** — an explicit `remove()` of a stale/expired entry, or
- **expiry** — a `extend_ttl` / TTL-bump call that keeps the entry's lifetime under
  the caller's control instead of defaulting to the max archival period, or
- **bounded-size check** — a visible `.len()` comparison gating growth of a tracked
  nullifier set.

Left unbounded, the nullifier store's ledger footprint grows linearly with usage
forever, inflating rent costs and eventually pushing the contract toward Soroban's
ledger-size limits — a slow-burn denial-of-service distinct from a single
oversized write.

This is a sibling of [`unbounded_storage`](unbounded_storage.md), not a duplicate:
`unbounded_storage` flags a single `Vec`/`Map` collection that is read, grown via
`push`/`insert`, and written back as a whole. A nullifier set is almost never
implemented that way (a growing `Vec` of every nullifier would itself be
prohibitively expensive to scan) — instead, contracts key each nullifier
individually (`storage().persistent().set(&nullifier, &true)`), which is exactly
the *keyed-entry* growth pattern `unbounded_storage` cannot see because no local
collection is ever grown. `nullifier_growth` targets that pattern specifically,
scoped to note-spending/proof-verification entrypoints.

## Vulnerable example

```rust
#[contractimpl]
impl ShieldedPool {
    // Marks a note's nullifier spent so it can't be replayed — but the entry
    // is never pruned, never TTL-extended, and nothing caps the set's size.
    pub fn spend_note(env: Env, nullifier: BytesN<32>, proof: BytesN<192>) {
        verify_note_proof(&env, &proof);
        env.storage().persistent().set(&nullifier, &true);
    }
}
```

## The fix

Nullifiers can't be deleted safely without reintroducing the double-spend they
exist to prevent, so "prune" here usually means bounding cost rather than
discarding correctness. Extend the entry's TTL explicitly (instead of relying on
the default archival window) so the rent cost is a conscious, tunable parameter,
and/or cap the tracked set with a visible length check when your design keeps an
auxiliary index:

```rust
#[contractimpl]
impl ShieldedPool {
    pub fn spend_note(env: Env, nullifier: BytesN<32>, proof: BytesN<192>) {
        verify_note_proof(&env, &proof);
        env.storage().persistent().set(&nullifier, &true);
        // Explicit, bounded TTL instead of an unmanaged default lifetime.
        env.storage().persistent().extend_ttl(&nullifier, 100_000, 500_000);
    }

    // Or: gate growth on a tracked-set size when the design keeps one.
    pub fn spend_note_capped(env: Env, nullifier: BytesN<32>, tracked: Vec<BytesN<32>>) {
        assert!(tracked.len() < 10_000_000, "nullifier set is full");
        env.storage().persistent().set(&nullifier, &true);
    }
}
```

## How Sanctifier detects it

The rule walks `#[contractimpl]` entrypoints whose name matches a spend/verify/
nullify/redeem/claim/withdraw hint, and looks for a `storage().persistent()` /
`instance()` `.set(&key, ...)` call whose key text reads like a nullifier or
commitment (`nullifier`, `commitment`, `null_hash`, `nf_`, ...). It flags the write
unless the same function also calls `.remove(...)` on a durable storage chain,
calls `.extend_ttl(...)` / a `*bump*`-named TTL helper, or reads a `.len()`
anywhere — any of which signal the author is actively bounding the entry's growth
or lifetime.

**Limitations:** both the entrypoint-name and key-name heuristics are naming
conventions, not semantic proofs — a nullifier argument named `id` or a spend
function named `process` will not be flagged; rename to match, or file a detector
issue for a naming pattern this misses. It also reasons about a single function,
so bounding logic implemented in a helper called from the entrypoint is a false
positive — add an explicit local `extend_ttl`/`.len()` call, or suppress with a
justification.

## References

- Soroban docs — [Persisting Data](https://soroban.stellar.org/docs/fundamentals-and-concepts/persisting-data)
- Soroban docs — [State Archival](https://soroban.stellar.org/docs/fundamentals-and-concepts/state-archival)
- [CWE-770: Allocation of Resources Without Limits or Throttling](https://cwe.mitre.org/data/definitions/770.html)
- Related: [`unbounded_storage`](unbounded_storage.md), [`missing_ttl`](missing_ttl.md)
