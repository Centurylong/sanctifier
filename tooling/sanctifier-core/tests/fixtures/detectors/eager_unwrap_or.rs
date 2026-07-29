use soroban_sdk::{contractimpl, Env, Address, String};

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn do_thing(env: Env, user: Address) {
        let name = get_name(&env, &user);

        // Expensive: function call
        let valid_name = name.clone().unwrap_or(compute_default_name(&env));

        // Expensive: macro
        let other_name = name.clone().unwrap_or(panic!("Should not happen"));

        // Expensive: method call
        let more_name = name.clone().unwrap_or(user.to_string());

        // Cheap: literal
        let count: Option<u32> = Some(5);
        let valid_count = count.unwrap_or(0);

        // Cheap: boolean literal
        let flag: Option<bool> = Some(true);
        let valid_flag = flag.unwrap_or(false);

        // Safe: lazily evaluated using unwrap_or_else
        let safe_name = name.unwrap_or_else(|| compute_default_name(&env));
    }
}

fn get_name(env: &Env, user: &Address) -> Option<String> {
    None
}

fn compute_default_name(env: &Env) -> String {
    String::from_str(env, "default")
}
