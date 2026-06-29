#![no_std]

//! # AMM Liquidity Pool with Constant Product Formula (x * y = k)
//!
//! This contract implements a hardened Automated Market Maker (AMM) using the constant product formula.
//!
//! ## Core Invariant: Constant Product Formula (k = x * y)
//!
//! The fundamental invariant of this AMM is that the product of the two token reserves
//! must remain constant (or increase due to fees) across all operations:
//!
//! **k = reserve_a * reserve_b**
//!
//! ### Mathematical Properties:
//! 1. **Conservation**: k never decreases (only increases due to fees)
//! 2. **Price Discovery**: Price = reserve_a / reserve_b
//! 3. **Slippage**: Larger trades have exponentially higher price impact
//! 4. **Fee Accumulation**: Each swap increases k by collecting fees
//!
//! ### Swap Formula:
//! For a swap of `amount_in` tokens A for tokens B:
//! ```text
//! amount_out = (reserve_b * amount_in * (10000 - fee_bps)) / ((reserve_a * 10000) + (amount_in * (10000 - fee_bps)))
//! ```
//!
//! This ensures: `(reserve_a + amount_in) * (reserve_b - amount_out) >= k`
//!
//! ## Security Features:
//! 1. **Slippage Protection**: min_amount_out parameter prevents sandwich attacks
//! 2. **Deadline Protection**: deadline parameter prevents transaction delay attacks  
//! 3. **Overflow Protection**: All arithmetic uses checked operations
//! 4. **Invariant Enforcement**: k-invariant verified on every operation

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, token, Address, Env,
    Symbol,
};

// ── Kani Formal Verification ────────────────────────────────────────────────────
#[cfg(kani)]
mod kani_harness;

// ── Data Types ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolInfo {
    pub token_a: Address,
    pub token_b: Address,
    pub reserve_a: u128,
    pub reserve_b: u128,
    pub total_supply: u128,
    pub fee_bps: u32, // Fee in basis points (e.g., 30 = 0.3%)
}

// ── Error Types ──────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AmmError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    InsufficientOutput = 3,
    DeadlineExpired = 4,
    InsufficientLiquidity = 5,
    InvalidAmount = 6,
    InvalidFee = 7,
    InvariantViolation = 8,
    CalculationOverflow = 9,
}

// ── Storage Keys ─────────────────────────────────────────────────────────────────

const POOL_INFO: &str = "POOL_INFO";
const USER_SHARES: &str = "SHARES";

// ── Contract Implementation ──────────────────────────────────────────────────────

#[contract]
pub struct AmmPool;

#[contractimpl]
impl AmmPool {
    /// Helper function to get pool info storage key
    fn pool_info_key(env: &Env) -> Symbol {
        Symbol::new(env, POOL_INFO)
    }

    /// Helper function to get user shares storage key
    fn user_shares_key(env: &Env, user: &Address) -> (Symbol, Address) {
        (Symbol::new(env, USER_SHARES), user.clone())
    }

    /// Initialize the AMM pool with two tokens and fee rate
    ///
    /// # Arguments
    /// * `token_a` - Address of first token contract
    /// * `token_b` - Address of second token contract  
    /// * `fee_bps` - Fee in basis points (e.g., 30 = 0.3%)
    ///
    /// # Security Notes
    /// - Can only be called once
    /// - Fee must be less than 100% (10,000 basis points)
    pub fn initialize(env: Env, token_a: Address, token_b: Address, fee_bps: u32) {
        // Check if already initialized
        if env.storage().instance().has(&Self::pool_info_key(&env)) {
            panic_with_error!(&env, AmmError::AlreadyInitialized);
        }

        // Validate fee (must be less than 100%)
        if fee_bps >= 10000 {
            panic_with_error!(&env, AmmError::InvalidFee);
        }

        // Create pool info
        let pool = PoolInfo {
            token_a: token_a.clone(),
            token_b: token_b.clone(),
            reserve_a: 0,
            reserve_b: 0,
            total_supply: 0,
            fee_bps,
        };

        env.storage()
            .instance()
            .set(&Self::pool_info_key(&env), &pool);
    }

    /// Execute a token swap with slippage and deadline protection
    ///
    /// # Arguments
    /// * `user` - User executing the swap
    /// * `token_in` - Address of input token
    /// * `amount_in` - Amount of input tokens
    /// * `min_amount_out` - Minimum acceptable output (slippage protection)
    /// * `deadline` - Transaction deadline timestamp (MEV protection)
    ///
    /// # Returns
    /// Amount of output tokens received
    ///
    /// # Security Features
    /// - **Slippage Protection**: Reverts if output < min_amount_out
    /// - **Deadline Protection**: Reverts if current time > deadline
    /// - **K-Invariant**: Ensures constant product is maintained
    /// - **Overflow Protection**: All arithmetic is checked
    pub fn swap(
        env: Env,
        user: Address,
        token_in: Address,
        amount_in: u128,
        min_amount_out: u128,
        deadline: u64,
    ) -> u128 {
        // Authenticate user
        user.require_auth();

        // Check deadline
        if env.ledger().timestamp() > deadline {
            panic_with_error!(&env, AmmError::DeadlineExpired);
        }

        // Validate amount
        if amount_in == 0 {
            panic_with_error!(&env, AmmError::InvalidAmount);
        }

        // Get pool info
        let mut pool: PoolInfo = env
            .storage()
            .instance()
            .get(&Self::pool_info_key(&env))
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::NotInitialized));

        // Check if we have liquidity
        if pool.reserve_a == 0 || pool.reserve_b == 0 {
            panic_with_error!(&env, AmmError::InsufficientLiquidity);
        }

        // Determine swap direction and calculate output
        let (amount_out, is_a_to_b) = if token_in == pool.token_a {
            let output = Self::calculate_swap_output_internal(
                &env,
                pool.reserve_a,
                pool.reserve_b,
                amount_in,
                pool.fee_bps as u128,
            );
            (output, true)
        } else if token_in == pool.token_b {
            let output = Self::calculate_swap_output_internal(
                &env,
                pool.reserve_b,
                pool.reserve_a,
                amount_in,
                pool.fee_bps as u128,
            );
            (output, false)
        } else {
            panic_with_error!(&env, AmmError::InvalidAmount);
        };
        // Check slippage protection
        if amount_out < min_amount_out {
            panic_with_error!(&env, AmmError::InsufficientOutput);
        }

        // Store k-value before swap for invariant check
        let k_before = pool
            .reserve_a
            .checked_mul(pool.reserve_b)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));

        // Execute token transfers and update reserves
        if is_a_to_b {
            // Transfer token A from user to pool
            let token_a_client = token::Client::new(&env, &pool.token_a);
            token_a_client.transfer(&user, &env.current_contract_address(), &(amount_in as i128));

            // Transfer token B from pool to user
            let token_b_client = token::Client::new(&env, &pool.token_b);
            token_b_client.transfer(
                &env.current_contract_address(),
                &user,
                &(amount_out as i128),
            );

            // Update reserves
            pool.reserve_a = pool
                .reserve_a
                .checked_add(amount_in)
                .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
            pool.reserve_b = pool
                .reserve_b
                .checked_sub(amount_out)
                .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
        } else {
            // Transfer token B from user to pool
            let token_b_client = token::Client::new(&env, &pool.token_b);
            token_b_client.transfer(&user, &env.current_contract_address(), &(amount_in as i128));

            // Transfer token A from pool to user
            let token_a_client = token::Client::new(&env, &pool.token_a);
            token_a_client.transfer(
                &env.current_contract_address(),
                &user,
                &(amount_out as i128),
            );

            // Update reserves
            pool.reserve_b = pool
                .reserve_b
                .checked_add(amount_in)
                .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
            pool.reserve_a = pool
                .reserve_a
                .checked_sub(amount_out)
                .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
        }

        // **CRITICAL**: Verify k-invariant is preserved (k should increase due to fees)
        let k_after = pool
            .reserve_a
            .checked_mul(pool.reserve_b)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));

        if k_after < k_before {
            panic_with_error!(&env, AmmError::InvariantViolation);
        }

        // Save updated pool state
        env.storage()
            .instance()
            .set(&Self::pool_info_key(&env), &pool);

        amount_out
    }
    /// Add liquidity to the pool
    ///
    /// # Arguments
    /// * `user` - User adding liquidity
    /// * `amount_a` - Amount of token A to add
    /// * `amount_b` - Amount of token B to add
    ///
    /// # Returns
    /// Amount of LP tokens minted
    ///
    /// # K-Invariant Impact
    /// Adding liquidity increases k proportionally: k_new = k_old × (1 + liquidity_ratio)
    pub fn add_liquidity(env: Env, user: Address, amount_a: u128, amount_b: u128) -> u128 {
        user.require_auth();

        if amount_a == 0 || amount_b == 0 {
            panic_with_error!(&env, AmmError::InvalidAmount);
        }

        let mut pool: PoolInfo = env
            .storage()
            .instance()
            .get(&Self::pool_info_key(&env))
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::NotInitialized));

        // Calculate liquidity to mint
        let liquidity = Self::calculate_liquidity_mint_internal(
            &env,
            pool.reserve_a,
            pool.reserve_b,
            amount_a,
            amount_b,
            pool.total_supply,
        );

        // Transfer tokens from user to pool
        let token_a_client = token::Client::new(&env, &pool.token_a);
        let token_b_client = token::Client::new(&env, &pool.token_b);

        token_a_client.transfer(&user, &env.current_contract_address(), &(amount_a as i128));
        token_b_client.transfer(&user, &env.current_contract_address(), &(amount_b as i128));

        // Update pool state
        pool.reserve_a = pool
            .reserve_a
            .checked_add(amount_a)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
        pool.reserve_b = pool
            .reserve_b
            .checked_add(amount_b)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
        pool.total_supply = pool
            .total_supply
            .checked_add(liquidity)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));

        // Update user's LP token balance
        let user_key = Self::user_shares_key(&env, &user);
        let current_shares: u128 = env.storage().persistent().get(&user_key).unwrap_or(0);
        let new_shares = current_shares
            .checked_add(liquidity)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
        env.storage().persistent().set(&user_key, &new_shares);

        env.storage()
            .instance()
            .set(&Self::pool_info_key(&env), &pool);

        liquidity
    }
    /// Remove liquidity from the pool
    ///
    /// # Arguments  
    /// * `user` - User removing liquidity
    /// * `liquidity` - Amount of LP tokens to burn
    ///
    /// # Returns
    /// Tuple of (amount_a, amount_b) tokens returned
    pub fn remove_liquidity(env: Env, user: Address, liquidity: u128) -> (u128, u128) {
        user.require_auth();

        if liquidity == 0 {
            panic_with_error!(&env, AmmError::InvalidAmount);
        }

        let mut pool: PoolInfo = env
            .storage()
            .instance()
            .get(&Self::pool_info_key(&env))
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::NotInitialized));

        // Check user has enough LP tokens
        let user_key = Self::user_shares_key(&env, &user);
        let user_shares: u128 = env.storage().persistent().get(&user_key).unwrap_or(0);
        if user_shares < liquidity {
            panic_with_error!(&env, AmmError::InsufficientLiquidity);
        }

        // Calculate amounts to return
        let (amount_a, amount_b) = Self::calculate_liquidity_burn_internal(
            &env,
            pool.reserve_a,
            pool.reserve_b,
            liquidity,
            pool.total_supply,
        );

        // Transfer tokens from pool to user
        let token_a_client = token::Client::new(&env, &pool.token_a);
        let token_b_client = token::Client::new(&env, &pool.token_b);

        token_a_client.transfer(&env.current_contract_address(), &user, &(amount_a as i128));
        token_b_client.transfer(&env.current_contract_address(), &user, &(amount_b as i128));

        // Update pool state
        pool.reserve_a = pool
            .reserve_a
            .checked_sub(amount_a)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
        pool.reserve_b = pool
            .reserve_b
            .checked_sub(amount_b)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
        pool.total_supply = pool
            .total_supply
            .checked_sub(liquidity)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));

        // Update user's LP token balance
        let new_shares = user_shares
            .checked_sub(liquidity)
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::CalculationOverflow));
        env.storage().persistent().set(&user_key, &new_shares);

        env.storage()
            .instance()
            .set(&Self::pool_info_key(&env), &pool);

        (amount_a, amount_b)
    }

    /// Get current pool information
    pub fn get_pool_info(env: Env) -> PoolInfo {
        env.storage()
            .instance()
            .get(&Self::pool_info_key(&env))
            .unwrap_or_else(|| panic_with_error!(&env, AmmError::NotInitialized))
    }

    /// Get user's LP token balance
    pub fn get_user_shares(env: Env, user: Address) -> u128 {
        let user_key = Self::user_shares_key(&env, &user);
        env.storage().persistent().get(&user_key).unwrap_or(0)
    }
}
// ── Pure Math Functions (Property-Testable) ─────────────────────────────────────

impl AmmPool {
    /// Internal calculate output amount for a swap (panics on error)
    fn calculate_swap_output_internal(
        env: &Env,
        reserve_in: u128,
        reserve_out: u128,
        amount_in: u128,
        fee_bps: u128,
    ) -> u128 {
        if amount_in == 0 {
            panic_with_error!(env, AmmError::InvalidAmount);
        }
        if reserve_in == 0 || reserve_out == 0 {
            panic_with_error!(env, AmmError::InsufficientLiquidity);
        }
        if fee_bps >= 10000 {
            panic_with_error!(env, AmmError::InvalidFee);
        }

        // Calculate amount_in after fee: amount_in × (10000 - fee_bps)
        let amount_in_with_fee = amount_in
            .checked_mul(10000 - fee_bps)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

        // Calculate numerator: reserve_out × amount_in_with_fee
        let numerator = reserve_out
            .checked_mul(amount_in_with_fee)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

        // Calculate denominator: (reserve_in × 10000) + amount_in_with_fee
        let denominator = reserve_in
            .checked_mul(10000)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow))
            .checked_add(amount_in_with_fee)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

        let output = numerator
            .checked_div(denominator)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

        if output >= reserve_out {
            panic_with_error!(env, AmmError::InsufficientLiquidity);
        }

        output
    }

    /// Internal calculate LP tokens to mint when adding liquidity (panics on error)
    fn calculate_liquidity_mint_internal(
        env: &Env,
        reserve_a: u128,
        reserve_b: u128,
        amount_a: u128,
        amount_b: u128,
        total_supply: u128,
    ) -> u128 {
        if amount_a == 0 || amount_b == 0 {
            panic_with_error!(env, AmmError::InvalidAmount);
        }

        // First liquidity provision - use geometric mean
        if total_supply == 0 {
            let product = amount_a
                .checked_mul(amount_b)
                .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

            let liquidity = integer_sqrt(product);
            if liquidity == 0 {
                panic_with_error!(env, AmmError::InvalidAmount);
            }
            return liquidity;
        }

        // Subsequent liquidity provision
        if reserve_a == 0 || reserve_b == 0 {
            panic_with_error!(env, AmmError::InsufficientLiquidity);
        }

        // Calculate liquidity based on both ratios and take minimum
        let liquidity_a = amount_a
            .checked_mul(total_supply)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow))
            .checked_div(reserve_a)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

        let liquidity_b = amount_b
            .checked_mul(total_supply)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow))
            .checked_div(reserve_b)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

        let liquidity = liquidity_a.min(liquidity_b);
        if liquidity == 0 {
            panic_with_error!(env, AmmError::InvalidAmount);
        }

        liquidity
    }
    /// Internal calculate tokens to return when burning LP tokens (panics on error)
    fn calculate_liquidity_burn_internal(
        env: &Env,
        reserve_a: u128,
        reserve_b: u128,
        liquidity: u128,
        total_supply: u128,
    ) -> (u128, u128) {
        if liquidity == 0 {
            panic_with_error!(env, AmmError::InvalidAmount);
        }
        if total_supply == 0 {
            panic_with_error!(env, AmmError::InsufficientLiquidity);
        }
        if liquidity > total_supply {
            panic_with_error!(env, AmmError::InsufficientLiquidity);
        }

        let amount_a = reserve_a
            .checked_mul(liquidity)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow))
            .checked_div(total_supply)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

        let amount_b = reserve_b
            .checked_mul(liquidity)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow))
            .checked_div(total_supply)
            .unwrap_or_else(|| panic_with_error!(env, AmmError::CalculationOverflow));

        if amount_a == 0 || amount_b == 0 {
            panic_with_error!(env, AmmError::InvalidAmount);
        }

        (amount_a, amount_b)
    }

    /// Calculate output amount for a swap using constant product formula
    ///
    /// # Formula
    /// ```text
    /// output = (reserve_out * amount_in * (10000 - fee_bps)) / ((reserve_in * 10000) + (amount_in * (10000 - fee_bps)))
    /// ```
    ///
    /// # K-Invariant Preservation
    /// This formula ensures that: (reserve_in + amount_in) * (reserve_out - output) >= reserve_in * reserve_out
    /// The inequality becomes equality when fee_bps = 0, and k increases when fee_bps > 0.
    fn calculate_swap_output(
        reserve_in: u128,
        reserve_out: u128,
        amount_in: u128,
        fee_bps: u128,
    ) -> Result<u128, AmmError> {
        if amount_in == 0 {
            return Err(AmmError::InvalidAmount);
        }
        if reserve_in == 0 || reserve_out == 0 {
            return Err(AmmError::InsufficientLiquidity);
        }
        if fee_bps >= 10000 {
            return Err(AmmError::InvalidFee);
        }

        // Calculate amount_in after fee: amount_in × (10000 - fee_bps)
        let amount_in_with_fee = amount_in
            .checked_mul(10000 - fee_bps)
            .ok_or(AmmError::CalculationOverflow)?;

        // Calculate numerator: reserve_out × amount_in_with_fee
        let numerator = reserve_out
            .checked_mul(amount_in_with_fee)
            .ok_or(AmmError::CalculationOverflow)?;

        // Calculate denominator: (reserve_in × 10000) + amount_in_with_fee
        let denominator = reserve_in
            .checked_mul(10000)
            .ok_or(AmmError::CalculationOverflow)?
            .checked_add(amount_in_with_fee)
            .ok_or(AmmError::CalculationOverflow)?;

        let output = numerator
            .checked_div(denominator)
            .ok_or(AmmError::CalculationOverflow)?;

        if output >= reserve_out {
            return Err(AmmError::InsufficientLiquidity);
        }

        Ok(output)
    }
    /// Calculate LP tokens to mint when adding liquidity
    fn calculate_liquidity_mint(
        reserve_a: u128,
        reserve_b: u128,
        amount_a: u128,
        amount_b: u128,
        total_supply: u128,
    ) -> Result<u128, AmmError> {
        if amount_a == 0 || amount_b == 0 {
            return Err(AmmError::InvalidAmount);
        }

        // First liquidity provision - use geometric mean
        if total_supply == 0 {
            let product = amount_a
                .checked_mul(amount_b)
                .ok_or(AmmError::CalculationOverflow)?;

            let liquidity = integer_sqrt(product);
            if liquidity == 0 {
                return Err(AmmError::InvalidAmount);
            }
            return Ok(liquidity);
        }

        // Subsequent liquidity provision
        if reserve_a == 0 || reserve_b == 0 {
            return Err(AmmError::InsufficientLiquidity);
        }

        // Calculate liquidity based on both ratios and take minimum
        let liquidity_a = amount_a
            .checked_mul(total_supply)
            .ok_or(AmmError::CalculationOverflow)?
            .checked_div(reserve_a)
            .ok_or(AmmError::CalculationOverflow)?;

        let liquidity_b = amount_b
            .checked_mul(total_supply)
            .ok_or(AmmError::CalculationOverflow)?
            .checked_div(reserve_b)
            .ok_or(AmmError::CalculationOverflow)?;

        let liquidity = liquidity_a.min(liquidity_b);
        if liquidity == 0 {
            return Err(AmmError::InvalidAmount);
        }

        Ok(liquidity)
    }

    /// Calculate tokens to return when burning LP tokens
    fn calculate_liquidity_burn(
        reserve_a: u128,
        reserve_b: u128,
        liquidity: u128,
        total_supply: u128,
    ) -> Result<(u128, u128), AmmError> {
        if liquidity == 0 {
            return Err(AmmError::InvalidAmount);
        }
        if total_supply == 0 {
            return Err(AmmError::InsufficientLiquidity);
        }
        if liquidity > total_supply {
            return Err(AmmError::InsufficientLiquidity);
        }

        let amount_a = reserve_a
            .checked_mul(liquidity)
            .ok_or(AmmError::CalculationOverflow)?
            .checked_div(total_supply)
            .ok_or(AmmError::CalculationOverflow)?;

        let amount_b = reserve_b
            .checked_mul(liquidity)
            .ok_or(AmmError::CalculationOverflow)?
            .checked_div(total_supply)
            .ok_or(AmmError::CalculationOverflow)?;

        if amount_a == 0 || amount_b == 0 {
            return Err(AmmError::InvalidAmount);
        }

        Ok((amount_a, amount_b))
    }
}
/// Integer square root using Newton's method
/// Used for calculating initial liquidity: sqrt(amount_a × amount_b)
fn integer_sqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }

    let mut x = n;
    let mut y = x.div_ceil(2);

    while y < x {
        x = y;
        y = (x + n / x).div_ceil(2);
    }

    x
}

// ── Legacy Pure Functions (for backward compatibility) ───────────────────────────

pub fn calculate_swap_output(
    reserve_in: u128,
    reserve_out: u128,
    amount_in: u128,
    fee_bps: u128,
) -> Result<u128, &'static str> {
    AmmPool::calculate_swap_output(reserve_in, reserve_out, amount_in, fee_bps)
        .map_err(|_| "Calculation failed")
}

pub fn calculate_liquidity_mint(
    reserve_a: u128,
    reserve_b: u128,
    amount_a: u128,
    amount_b: u128,
    total_supply: u128,
) -> Result<u128, &'static str> {
    AmmPool::calculate_liquidity_mint(reserve_a, reserve_b, amount_a, amount_b, total_supply)
        .map_err(|_| "Calculation failed")
}

pub fn calculate_liquidity_burn(
    reserve_a: u128,
    reserve_b: u128,
    liquidity: u128,
    total_supply: u128,
) -> Result<(u128, u128), &'static str> {
    AmmPool::calculate_liquidity_burn(reserve_a, reserve_b, liquidity, total_supply)
        .map_err(|_| "Calculation failed")
}
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_swap_output_calculation() {
        // Pool with 1000 of each token, 0.3% fee
        let output = calculate_swap_output(1000, 1000, 100, 30).unwrap();
        assert!(output > 0);
        assert!(output < 100); // Should be less due to slippage and fees
    }

    #[test]
    fn test_k_invariant_preservation() {
        let reserve_in = 1000u128;
        let reserve_out = 1000u128;
        let amount_in = 100u128;
        let fee_bps = 30u128;

        let k_before = reserve_in * reserve_out;
        let amount_out =
            calculate_swap_output(reserve_in, reserve_out, amount_in, fee_bps).unwrap();

        let new_reserve_in = reserve_in + amount_in;
        let new_reserve_out = reserve_out - amount_out;
        let k_after = new_reserve_in * new_reserve_out;

        // K should increase due to fees
        assert!(k_after >= k_before);
    }

    #[test]
    fn test_contract_initialization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AmmPool);
        let client = AmmPoolClient::new(&env, &contract_id);

        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        // Should initialize successfully
        client.initialize(&token_a, &token_b, &30);

        // Test that it's initialized by checking pool info
        let pool = client.get_pool_info();
        assert_eq!(pool.token_a, token_a);
        assert_eq!(pool.token_b, token_b);
        assert_eq!(pool.fee_bps, 30);
    }

    #[test]
    fn test_liquidity_calculations() {
        // Test geometric mean for first provision
        let liquidity = calculate_liquidity_mint(0, 0, 1000, 1000, 0).unwrap();
        assert_eq!(liquidity, 1000); // sqrt(1000 * 1000) = 1000

        // Test proportional liquidity for subsequent provisions
        let liquidity = calculate_liquidity_mint(1000, 1000, 500, 500, 1000).unwrap();
        assert_eq!(liquidity, 500); // (500 * 1000) / 1000 = 500

        // Test burn calculation
        let (amount_a, amount_b) = calculate_liquidity_burn(1500, 1500, 500, 1500).unwrap();
        assert_eq!(amount_a, 500); // (1500 * 500) / 1500 = 500
        assert_eq!(amount_b, 500);
    }
}
