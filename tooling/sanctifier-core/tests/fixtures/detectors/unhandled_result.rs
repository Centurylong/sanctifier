#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: unhandled_result detector
// A public, non-Result-returning function ignores a Result-returning call.

fn persist(_env: &Env) -> Result<(), Error> {
    Ok(())
}

pub enum Error {
    Failed,
}

#[contract]
pub struct UnhandledResultContract;

#[contractimpl]
impl UnhandledResultContract {
    // Violation: return value of persist() is discarded.
    pub fn save(env: Env) {
        persist(&env);
    }

    // Clean: the Result is propagated with `?`.
    pub fn save_checked(env: Env) -> Result<(), Error> {
        persist(&env)?;
        Ok(())
    }
}
