# `hardcoded_decimals` — Fixed token precision assumptions

| | |
| --- | --- |
| **Finding code** | [`SANCT_HARDCODED_DECIMALS`](../error-codes.md) |
| **Category** | arithmetic |
| **Severity** | Warning |
| **Source rule** | [`rules/hardcoded_decimals.rs`](../../tooling/sanctifier-core/src/rules/hardcoded_decimals.rs) |

## What it catches

Token amounts are integer values whose meaning depends on the asset's decimal precision. A contract or SDK helper that assumes every asset uses the same decimal count can silently over- or under-scale balances when it receives a token with different precision.

The detector flags precision-shaped constants such as `TOKEN_DECIMALS = 7` and powers-of-ten used to scale token quantities in conversion code. It deliberately avoids rate-shaped arithmetic such as basis points, fees, percentages, commissions, and interest.

## Vulnerable example

```rust
const TOKEN_DECIMALS: u32 = 7;

pub fn display_balance(balance: i128) -> i128 {
    balance / 10_000_000
}
```

Both values assume seven decimal places regardless of the actual asset.

## The fix

Read the precision from the asset and derive the scale from that value:

```rust
pub fn display_balance(asset: TokenClient, amount: i128) -> i128 {
    let decimals = asset.decimals();
    let scale = 10_i128.pow(decimals);
    amount / scale
}
```

## How Sanctifier detects it

The rule combines narrow naming and arithmetic context:

1. decimal/precision/scale bindings are inspected for fixed integer literals;
2. powers of ten are recognized as likely decimal scales;
3. multiplication or division by a power of ten is reported only when token-quantity identifiers and conversion-shaped function/local names provide context; and
4. rate-shaped contexts such as `bps`, `fee`, `interest`, `percent`, `commission`, and `ratio` are excluded.

This is an advisory source heuristic, not type-level proof that a value represents an asset amount. Dynamic `asset.decimals()`-derived scales are intentionally not reported.
