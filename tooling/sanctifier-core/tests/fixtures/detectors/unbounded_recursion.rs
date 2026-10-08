use soroban_sdk::{contractimpl, Env};

pub struct Recursor;

#[contractimpl]
impl Recursor {
    pub fn recurse_forever(env: Env, value: u32) {
        if value > 0 {
            Self::recurse_forever(env, value - 1);
        } else {
            Self::recurse_forever(env, value);
        }
    }

    pub fn bounded_depth(env: Env, value: u32, depth: u32) {
        if depth >= 8 {
            return;
        }
        Self::bounded_depth(env, value, depth + 1);
    }

    pub fn bounded_countdown(env: Env, depth: u32) {
        if depth == 0 {
            return;
        }
        Self::bounded_countdown(env, depth - 1);
    }

    pub fn same_name_on_other_value(helper: Helper) {
        helper.same_name_on_other_value();
    }

    pub fn same_name_on_other_type(env: Env) {
        Helper::same_name_on_other_type(env);
    }
}
