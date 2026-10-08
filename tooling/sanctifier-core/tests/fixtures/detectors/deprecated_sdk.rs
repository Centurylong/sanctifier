//! Mixed fixture for the deprecated_sdk detector.

use soroban_sdk::{Env, String, Symbol};

pub fn legacy(env: Env) {
    env.logger();
    env.logs().log("legacy", &[]);
    env.prng().u64_in_range(1..=10);
    let _ = Symbol::short("legacy");
    let _ = String::from_slice(&env, "legacy");
    panic_error!(&env, Error::Legacy);
}

pub fn renamed_ctx(ctx: Env) {
    ctx.logger();
}

pub fn modern(env: Env) {
    env.logs();
    env.logs().add("modern", &[]);
    env.prng().gen_range(1..=10);
    let _ = symbol_short!("modern");
    let _ = String::from_str(&env, "modern");
    panic_with_error!(&env, Error::Modern);
}
