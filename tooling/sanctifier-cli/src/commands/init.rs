use clap::Args;
use colored::Colorize;
use sanctifier_core::{CustomRule, SanctifyConfig};
use std::fs;
use std::path::{Path, PathBuf};

/// Contract template to scaffold with `sanctifier init --template <TEMPLATE>`.
#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum Template {
    /// SEP-41 fungible token with auth, overflow, and supply-cap checks baked in
    Token,
    /// Constant-product AMM with slippage guard and K-invariant enforcement
    Amm,
    /// M-of-N multi-signature governance with timelock and nonce replay protection
    Multisig,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Force overwrite existing configuration file
    #[arg(short, long)]
    pub force: bool,

    /// Contract template to scaffold (token | amm | multisig)
    #[arg(long, value_enum)]
    pub template: Option<Template>,

    /// Output directory for the scaffolded contract [default: current dir]
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

pub struct ConfigGenerator;

impl ConfigGenerator {
    pub fn generate_default_config() -> SanctifyConfig {
        SanctifyConfig {
            ignore_paths: vec!["target".to_string(), ".git".to_string()],
            enabled_rules: vec![
                "auth_gaps".to_string(),
                "panics".to_string(),
                "arithmetic".to_string(),
                "ledger_size".to_string(),
            ],
            ledger_limit: 64000,
            strict_mode: false,
            custom_rules: vec![
                CustomRule {
                    name: "no_unsafe_block".to_string(),
                    pattern: "unsafe\\s*\\{".to_string(),
                    severity: sanctifier_core::RuleSeverity::Error,
                },
                CustomRule {
                    name: "no_mem_forget".to_string(),
                    pattern: "std::mem::forget".to_string(),
                    severity: sanctifier_core::RuleSeverity::Warning,
                },
            ],
            approaching_threshold: 0.8,
        }
    }
}

pub struct FileWriter;

impl FileWriter {
    pub fn config_exists(path: &Path) -> bool {
        path.join(".sanctify.toml").exists()
    }

    pub fn write_config(config: &SanctifyConfig, path: &Path) -> anyhow::Result<PathBuf> {
        let config_path = path.join(".sanctify.toml");
        let toml_string = toml::to_string_pretty(config)?;
        fs::write(&config_path, toml_string)?;
        Ok(config_path)
    }
}

pub struct OutputFormatter;

impl OutputFormatter {
    pub fn display_success(config_path: &Path) {
        println!("{} Configuration file created successfully!", "\u2713".green());
        println!("   Location: {}", config_path.display());
    }

    pub fn display_existing_file_warning() {
        eprintln!(
            "{} Configuration file already exists: .sanctify.toml",
            "\u26a0".yellow()
        );
        eprintln!("   Use --force to overwrite the existing configuration");
    }

    pub fn display_error(error: &anyhow::Error) {
        eprintln!("{} Failed to create configuration file", "\u2717".red());
        eprintln!("   Error: {}", error);
    }

    pub fn display_template_hint() {
        println!();
        println!("   {} Scaffold a secure contract template with --template:", "\u2192".cyan());
        println!("     sanctifier init --template token     # SEP-41 fungible token");
        println!("     sanctifier init --template amm       # Constant-product AMM");
        println!("     sanctifier init --template multisig  # M-of-N governance");
    }
}
    pub fn display_scaffold_success(template_name: &str, output_dir: &Path) {
        println!("{} Scaffolded {} contract successfully!", "\u2713".green(), template_name.bold());
        println!("   Location: {}", output_dir.display());
    }

    pub fn display_scaffold_files(files: &[&Path]) {
        println!("   Files created:");
        for f in files {
            println!("     {} {}", "\u2022".green(), f.display());
        }
    }
}

pub struct TemplateGenerator;

impl TemplateGenerator {
    fn write_file(path: &Path, content: &str, force: bool) -> anyhow::Result<()> {
        if path.exists() && !force {
            anyhow::bail!(
                "file already exists: {} (use --force to overwrite)",
                path.display()
            );
        }
        fs::write(path, content)?;
        Ok(())
    
    fn cargo_toml(name: &str) -> String {
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
soroban-sdk = {{ version = "21.0.0" }}

[dev-dependencies]
soroban-sdk = {{ version = "21.0.0", features = ["testutils"] }}

[profile.release]
opt-level     = "z"
overflow-checks = true
debug         = false
strip         = "symbols"
panic         = "abort"
codegen-units = 1
lto           = true
"#,
            name = name
        )
    
    fn token_contract() -> &'static str {
        r##"#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String as SorobanString};

// SECURITY: Supply cap enforced at compile time — prevents unbounded inflation attacks.
// Adjust before deployment; cannot be changed post-deploy without a contract upgrade.
const MAX_SUPPLY: i128 = 1_000_000_000 * 10_i128.pow(7); // 1 billion tokens, 7 decimals

#[contracttype]
pub enum DataKey {
    Balance(Address),
    Allowance(Address, Address),
    TotalSupply,
    Admin,
    Name,
    Symbol,
}

/// SEP-41 fungible token with security checks baked in.
///
/// `// #[sanctify(...)]` annotations are parsed by `sanctifier analyze` and
/// verified against the rules configured in .sanctify.toml.
#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    /// One-time initialiser. Panics if already called — prevents re-initialisation.
    // #[sanctify(auth = "admin", once = true)]
    pub fn initialize(env: Env, admin: Address, name: SorobanString, symbol: SorobanString) {
        // SECURITY: re-init guard — must be the first state mutation check
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("contract already initialised"); // #[sanctify(panic)] expected guard
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TotalSupply, &0_i128);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
    }

    /// Mint `amount` tokens to `to`. Admin-only.
    // #[sanctify(auth = "admin", arithmetic)]
    pub fn mint(env: Env, to: Address, amount: i128) {
        assert!(amount > 0, "amount must be positive");
        Self::require_admin(&env); // SECURITY: auth BEFORE state mutation — #[sanctify(auth_gaps)]
        let supply: i128 = env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0);
        // SECURITY: checked_add to detect overflow before writing state
        let new_supply = supply.checked_add(amount).expect("arithmetic overflow"); // #[sanctify(arithmetic)]
        assert!(new_supply <= MAX_SUPPLY, "supply cap exceeded"); // #[sanctify(invariants)]
        let bal: i128 = env.storage().persistent()
            .get(&DataKey::Balance(to.clone())).unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::Balance(to),
            &bal.checked_add(amount).expect("arithmetic overflow"),
        );
        env.storage().instance().set(&DataKey::TotalSupply, &new_supply);
    }

    /// Transfer `amount` from `from` to `to`. Sender must authorise.
    // #[sanctify(auth = "from", arithmetic)]
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        assert!(amount > 0, "amount must be positive");
        from.require_auth(); // SECURITY: auth BEFORE state read — #[sanctify(auth_gaps)]
        let from_bal: i128 = env.storage().persistent()
            .get(&DataKey::Balance(from.clone())).unwrap_or(0);
        assert!(from_bal >= amount, "insufficient balance");
        let to_bal: i128 = env.storage().persistent()
            .get(&DataKey::Balance(to.clone())).unwrap_or(0);
        // Checks-effects: update state only after all assertions pass
        env.storage().persistent().set(&DataKey::Balance(from), &(from_bal - amount));
        env.storage().persistent().set(
            &DataKey::Balance(to),
            &to_bal.checked_add(amount).expect("arithmetic overflow"),
        );
    }

    /// Approve `spender` to transfer up to `amount` on behalf of `from`.
    // #[sanctify(auth = "from")]
    pub fn approve(env: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth(); // #[sanctify(auth_gaps)]
        env.storage().persistent().set(&DataKey::Allowance(from, spender), &amount);
    }

    pub fn balance(env: Env, account: Address) -> i128 {
        env.storage().persistent().get(&DataKey::Balance(account)).unwrap_or(0)
    }

    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0)
    }

    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).expect("not initialised")
    }

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not init");
        admin.require_auth();
    }
}
"##
    }
}
