#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Address};

#[contract]
pub struct RecursiveContract;

#[contractimpl]
impl RecursiveContract {
    /// Direct recursion - factorial calculation
    pub fn factorial(env: Env, n: u32) -> u32 {
        if n <= 1 {
            1
        } else {
            n * Self::factorial(env, n - 1)
        }
    }

    /// Direct recursion - fibonacci
    pub fn fibonacci(env: Env, n: u32) -> u32 {
        if n <= 1 {
            n
        } else {
            Self::fibonacci(env.clone(), n - 1) + Self::fibonacci(env, n - 2)
        }
    }

    /// Indirect recursion - function A calls B, B calls A
    pub fn process_a(env: Env, n: u32) -> u32 {
        if n > 0 {
            Self::process_b(env, n)
        } else {
            0
        }
    }

    fn process_b(env: Env, n: u32) -> u32 {
        if n > 1 {
            Self::process_a(env, n - 1)
        } else {
            1
        }
    }

    /// Non-recursive function for comparison
    pub fn add(env: Env, a: u32, b: u32) -> u32 {
        a + b
    }

    /// Non-recursive function that calls another non-recursive function
    pub fn multiply_and_add(env: Env, a: u32, b: u32, c: u32) -> u32 {
        let product = Self::multiply(a, b);
        Self::add(env, product, c)
    }

    fn multiply(a: u32, b: u32) -> u32 {
        a * b
    }
}
