use soroban_sdk::Env;

struct Contract;
struct FallbackBuilder;

impl Contract {
    pub fn eager_fn_call(env: Env, existing: Option<i128>) -> i128 {
        existing.unwrap_or(expensive_default(&env))
    }

    pub fn eager_method_call(env: Env, existing: Option<i128>) -> i128 {
        let fallback = FallbackBuilder::new(&env);
        existing.unwrap_or(fallback.build())
    }

    pub fn eager_block(env: Env, existing: Option<i128>) -> i128 {
        existing.unwrap_or({
            let value = expensive_default(&env);
            value
        })
    }

    pub fn cheap_literal(existing: Option<i128>) -> i128 {
        existing.unwrap_or(0)
    }

    pub fn cheap_identifier(existing: Option<i128>, fallback: i128) -> i128 {
        existing.unwrap_or(fallback)
    }

    fn private_helper(env: Env, existing: Option<i128>) -> i128 {
        existing.unwrap_or(expensive_default(&env))
    }
}

impl FallbackBuilder {
    fn new(_env: &Env) -> Self {
        Self
    }

    fn build(self) -> i128 {
        1
    }
}

fn expensive_default(_env: &Env) -> i128 {
    10
}
