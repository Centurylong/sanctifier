#![no_std]

// FIXTURE: hardcoded_decimals detector
// Fixed token precision assumptions fail when an asset exposes a different
// number of decimal places.

const TOKEN_DECIMALS: u32 = 7;
const BPS_SCALE: i128 = 10_000;

pub struct PrecisionContract;

impl PrecisionContract {
    // Violation: a token-specific decimal count is fixed at compile time.
    pub fn configured_decimals() -> u32 {
        TOKEN_DECIMALS
    }

    // Violation: display conversion assumes seven decimal places.
    pub fn display_balance(balance: i128) -> i128 {
        balance / 10_000_000
    }

    // Violation: `rate` is only a substring of `generate`, not a rate context.
    pub fn generate_units(balance: i128) -> i128 {
        balance / 10_000_000
    }

    // Safe: derive the decimal count and scale from the asset at runtime.
    pub fn display_dynamic(asset: TokenClient, amount: i128) -> i128 {
        let decimals = asset.decimals();
        let scale = 10_i128.pow(decimals);
        amount / scale
    }

    // Safe: basis-point fee math is a rate calculation, not asset precision.
    pub fn charge_fee(amount: i128) -> i128 {
        amount * 30 / BPS_SCALE
    }
}
