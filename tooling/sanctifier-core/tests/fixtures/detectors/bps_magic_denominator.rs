const BASIS_POINTS_DENOMINATOR: i128 = 10_000;

struct Contract;

impl Contract {
    pub fn hardcoded_fee(amount: i128, fee_bps: i128) -> i128 {
        amount * fee_bps / 10_000
    }

    pub fn suspicious_interest(principal: i128, interest_rate: i128) -> i128 {
        principal * interest_rate / 1_000
    }

    pub fn named_constant(amount: i128, fee_bps: i128) -> i128 {
        amount * fee_bps / BASIS_POINTS_DENOMINATOR
    }

    pub fn unrelated_progress(done: i128, total: i128) -> i128 {
        done * 100 / total
    }
}
