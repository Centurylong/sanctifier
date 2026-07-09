struct Contract;

impl Contract {
    pub fn settle_fee(amount: i128, rate: i128, denominator: i128) -> i128 {
        let maker = amount.mul_div_floor(rate, denominator);
        let taker = amount.mul_div_ceil(rate, denominator);
        maker + taker
    }

    pub fn deposit(amount: i128, rate: i128, denominator: i128) -> i128 {
        amount.mul_div_ceil(rate, denominator)
    }

    pub fn withdraw(shares: i128, rate: i128, denominator: i128) -> i128 {
        shares.mul_div_floor(rate, denominator)
    }

    pub fn claim_reward(amount: i128, rate: i128, denominator: i128) -> i128 {
        amount.mul_div_floor(rate, denominator)
    }

    pub fn layout(width: i128, denominator: i128) -> i128 {
        width.div_ceil(denominator)
    }
}
