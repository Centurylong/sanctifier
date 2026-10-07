# `ledger_size` — Ledger entry size risk

| | |
| --- | --- |
| **Finding code** | [`S004`](../error-codes.md) |
| **Category** | storage_limits |
| **Severity** | Medium |
| **Source rule** | [`rules/ledger_size.rs`](../../tooling/sanctifier-core/src/rules/ledger_size.rs) |
| **Glossary** | [Ledger entry size limit](../glossary.md#ledger-entry-size-limit) · [OOG](../glossary.md#oog-out-of-gas) |

## What it catches

A `#[contracttype]` struct or enum whose estimated serialized size approaches or
exceeds the ledger entry size limit (~64 KB by default, configurable with
`--limit`). Oversized entries are expensive to read/write and can make an
entrypoint **impossible to call** once the data grows, effectively bricking the
contract for that key.

## Vulnerable example

```rust
#[contracttype]
pub struct Registry {
    // An unbounded Vec inside a single ledger entry: grows until writes fail.
    pub entries: Vec<Record>,   // Record is itself large
    pub audit_log: Vec<String>, // append-only, never pruned
}
```

## The fix

Shard large collections across multiple keys so no single entry is unbounded:

```rust
#[contracttype]
pub enum DataKey {
    Record(u32),      // one ledger entry per record
    RecordCount,      // small counter entry
}

// Store each record under its own key instead of one giant Vec.
env.storage().persistent().set(&DataKey::Record(id), &record);
```

Keep hot, frequently-read metadata small; move append-only history off-chain or
into per-item keys.

## How Sanctifier detects it

The rule parses each `#[contracttype]` independently and computes an
**XDR-shaped payload estimate**, not a Rust in-memory `size_of` value. Fixed
width values include the `ScVal` type tag and XDR payload: small Rust integer
types widen to the 32-bit Soroban scalar representation, `u64`/`i64` and
128/256-bit values budget their wider XDR payloads, `Address` uses the larger
account-address representation, and `BytesN<N>` includes its padded fixed
byte payload. The repository is pinned to Soroban SDK 20.5.0, where raw Rust
arrays are not contract-spec field types; fixed byte payloads should use
`BytesN<N>`.

### Documented estimation margin

For fixed-width fields, Sanctifier applies a **one-sided +10% serialization
safety margin** to the calculated payload before comparing it with the ledger
entry budget. This is the detector's documented error/safety margin for
thresholding; it intentionally biases toward warning before the protocol cap
because a stored value is only part of the complete serialized ledger entry.

Every finding reports a per-type budget:

- raw estimated bytes;
- bytes after the +10% safety margin;
- remaining bytes before the configured limit;
- the configured limit itself.

The default near-cap threshold remains 80% of the configured limit. The safety
margin is applied first, so a type whose budgeted size crosses 80% is reported
as `ApproachingLimit`, while a budgeted size at or above the limit is
`ExceedsLimit`.

**Dynamic-size limitation:** `Bytes`, `String`, `Symbol`, `Vec`, and `Map`
remain runtime-sized and use a representative payload/one-element growth floor.
Same-file `#[contracttype]` structs and enums are now resolved recursively, so
nested fixed-width fields contribute their calculated estimate. Unknown external
types and cyclic references retain the 40-byte user-defined-type fallback.
The +10% margin is **not** a universal upper error bound for runtime-sized
collections. Pair this rule with [`unbounded_storage`](unbounded_storage.md)
when collection growth is the risk.

The golden `ledger_size` fixture uses `BytesN<4096>`, a contract-spec type
supported by the pinned SDK, and verifies that a modest fixed byte payload stays
below the warning threshold. Focused unit coverage uses larger valid
`BytesN<N>` values to exercise both the near-cap warning and over-cap error.
Raw Rust arrays such as `[u8; N]` are deliberately not used as serialization
fidelity evidence because SDK 20.5.0 rejects them in contract specs.

## References

- Stellar docs — [Storage strategies in production contracts](https://developers.stellar.org/docs/build/guides/storage/storage-strategies)
- Stellar XDR — [`SCVal` layout](https://github.com/stellar/stellar-xdr/blob/main/Stellar-contract.x)
- [CWE-770: Allocation of Resources Without Limits](https://cwe.mitre.org/data/definitions/770.html)
- Related: [`missing_ttl`](missing_ttl.md), [`arg_dos`](arg_dos.md)
