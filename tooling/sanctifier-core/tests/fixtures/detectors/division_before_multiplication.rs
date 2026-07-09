struct Contract;

impl Contract {
    pub fn payout(amount: i128, total_shares: i128, pool_balance: i128) -> i128 {
        amount / total_shares * pool_balance
    }

    pub fn fee(balance: i128, denominator: i128, rate_bps: i128) -> i128 {
        (balance / denominator) * rate_bps
    }

    pub fn precise_fee(amount: i128, rate_bps: i128, denominator: i128) -> i128 {
        amount * rate_bps / denominator
    }

    pub fn layout(width: i128, columns: i128, rows: i128) -> i128 {
        width / columns * rows
    }
}
