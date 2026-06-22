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
    
    fn token_config() -> &'static str {
        r#"# Generated by: sanctifier init --template token
# SEP-41 token security profile — strict mode enabled (zero tolerance for auth gaps)
ignore_paths          = ["target", ".git"]
enabled_rules         = ["auth_gaps", "panics", "arithmetic", "ledger_size", "invariants"]
ledger_limit          = 64000
strict_mode           = true
approaching_threshold = 0.8

[[custom_rules]]
name     = "no_unsafe_block"
pattern  = "unsafe"
severity = "error"

[[custom_rules]]
name     = "require_checked_arithmetic"
pattern  = "checked_add|checked_mul|checked_sub"
severity = "warning"

[[custom_rules]]
name     = "no_mem_forget"
pattern  = "std::mem::forget"
severity = "warning"
"#
    
    fn amm_contract() -> &'static str {
        r##"#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env};

// SECURITY: Fee is locked at 0.3% (30 bps) and cannot change after deployment.
// Dynamic fees would allow the admin to drain LPs by setting fee = 100%.
const FEE_BPS: i128  = 30;
const BPS_DENOM: i128 = 10_000;

#[contracttype]
pub enum DataKey {
    ReserveA, ReserveB, TokenA, TokenB,
    TotalShares, Shares(Address), Admin,
}

/// Constant-product AMM (x·y = k) with slippage guard and K-invariant enforcement.
///
/// The K-invariant assertion (`new_k >= old_k`) is the core security property.
/// Any swap that reduces the pool product is immediately rejected.
// #[sanctify(invariant = "k_invariant")]
#[contract]
pub struct AmmPool;

#[contractimpl]
impl AmmPool {
    // #[sanctify(auth = "admin", once = true)]
    pub fn initialize(env: Env, admin: Address, token_a: Address, token_b: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("pool already initialised"); // #[sanctify(panic)]
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenA, &token_a);
        env.storage().instance().set(&DataKey::TokenB, &token_b);
        env.storage().instance().set(&DataKey::ReserveA, &0_i128);
        env.storage().instance().set(&DataKey::ReserveB, &0_i128);
        env.storage().instance().set(&DataKey::TotalShares, &0_i128);
    }

    /// Swap `amount_in` of `token_in` for the other pool token.
    /// Reverts if output is below `min_out` — the caller-supplied slippage bound.
    // #[sanctify(auth = "caller", arithmetic, invariant = "k_invariant")]
    pub fn swap(env: Env, caller: Address, token_in: Address, amount_in: i128, min_out: i128) -> i128 {
        assert!(amount_in > 0, "amount_in must be positive");
        caller.require_auth(); // SECURITY: auth BEFORE state read — #[sanctify(auth_gaps)]

        let token_a: Address = env.storage().instance().get(&DataKey::TokenA).expect("not init");
        let token_b: Address = env.storage().instance().get(&DataKey::TokenB).expect("not init");
        let reserve_a: i128  = env.storage().instance().get(&DataKey::ReserveA).unwrap_or(0);
        let reserve_b: i128  = env.storage().instance().get(&DataKey::ReserveB).unwrap_or(0);
        assert!(reserve_a > 0 && reserve_b > 0, "pool has no liquidity");

        let (reserve_in, reserve_out, token_out) = if token_in == token_a {
            (reserve_a, reserve_b, token_b.clone())
        } else if token_in == token_b {
            (reserve_b, reserve_a, token_a.clone())
        } else {
            panic!("token_in not in pool"); // #[sanctify(panic)]
        };

        // Constant-product formula with fee deducted from input
        // amount_out = (amount_in * fee * reserve_out) / (reserve_in * BPS + amount_in * fee)
        let fee_num     = BPS_DENOM - FEE_BPS;
        let in_with_fee = amount_in.checked_mul(fee_num).expect("overflow"); // #[sanctify(arithmetic)]
        let numerator   = in_with_fee.checked_mul(reserve_out).expect("overflow");
        let denominator = reserve_in.checked_mul(BPS_DENOM).expect("overflow")
                            .checked_add(in_with_fee).expect("overflow");
        let amount_out  = numerator / denominator;

        // SECURITY: slippage guard — revert if output below caller's minimum
        assert!(amount_out >= min_out, "slippage: output below min_out"); // #[sanctify(invariants)]
        assert!(amount_out > 0, "output amount is zero");

        let old_k = reserve_a.checked_mul(reserve_b).expect("overflow");
        let (new_ra, new_rb) = if token_in == token_a {
            (reserve_a.checked_add(amount_in).expect("overflow"),
             reserve_b.checked_sub(amount_out).expect("underflow"))
        } else {
            (reserve_a.checked_sub(amount_out).expect("underflow"),
             reserve_b.checked_add(amount_in).expect("overflow"))
        };
        // SECURITY: K-invariant — pool product must not decrease after swap
        let new_k = new_ra.checked_mul(new_rb).expect("overflow");
        assert!(new_k >= old_k, "K-invariant violated"); // #[sanctify(invariants)]

        token::Client::new(&env, &token_in).transfer(&caller, &env.current_contract_address(), &amount_in);
        token::Client::new(&env, &token_out).transfer(&env.current_contract_address(), &caller, &amount_out);

        env.storage().instance().set(&DataKey::ReserveA, &new_ra);
        env.storage().instance().set(&DataKey::ReserveB, &new_rb);
        amount_out
    }

    pub fn get_reserves(env: Env) -> (i128, i128) {
        (
            env.storage().instance().get(&DataKey::ReserveA).unwrap_or(0),
            env.storage().instance().get(&DataKey::ReserveB).unwrap_or(0),
        )
    }
}
"##
    
    fn amm_config() -> &'static str {
        r#"# Generated by: sanctifier init --template amm
# Constant-product AMM security profile
ignore_paths          = ["target", ".git"]
enabled_rules         = ["auth_gaps", "panics", "arithmetic", "ledger_size", "invariants"]
ledger_limit          = 64000
strict_mode           = false
approaching_threshold = 0.8

[[custom_rules]]
name     = "no_unsafe_block"
pattern  = "unsafe"
severity = "error"

[[custom_rules]]
name     = "require_slippage_guard"
pattern  = "min_out"
severity = "error"

[[custom_rules]]
name     = "require_k_invariant_check"
pattern  = "K-invariant"
severity = "error"
"#
    
    fn multisig_contract() -> &'static str {
        r##"#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, BytesN, Env, Vec};

// SECURITY: 24-hour timelock prevents flash-loan governance attacks.
// A proposal submitted and executed in the same ledger is impossible.
const TIMELOCK_SECONDS: u64 = 86_400;

// SECURITY: Threshold is hardcoded at deploy time. Cannot be lowered post-deploy
// to prevent a governance takeover that reduces the required signers to 1.
const THRESHOLD: u32 = 2;

#[contracttype]
pub enum DataKey {
    Signers,
    Proposal(BytesN<32>),
    Nonce(u64),
    ApprovalCount(BytesN<32>),
    HasSigned(BytesN<32>, Address),
    ExecutedAt(BytesN<32>),
    UnlockTime(BytesN<32>),
}

#[contracttype]
pub struct Proposal {
    pub target:     Address,
    pub proposer:   Address,
    pub created_at: u64,
    pub nonce:      u64,
}

/// M-of-N multi-signature governance with timelock and nonce replay protection.
///
/// Core security properties enforced by sanctifier rules:
///   1. Nonce uniqueness — each nonce usable exactly once  [require_nonce_check]
///   2. Timelock         — execute reverts before unlock   [require_timelock]
///   3. Threshold        — execute requires >= THRESHOLD approvals
///   4. Signer-only      — submit/approve/execute gated to registered signers
// #[sanctify(invariant = "threshold_check")]
#[contract]
pub struct Multisig;

#[contractimpl]
impl Multisig {
    // #[sanctify(once = true)]
    pub fn initialize(env: Env, signers: Vec<Address>) {
        if env.storage().instance().has(&DataKey::Signers) {
            panic!("already initialised"); // #[sanctify(panic)]
        }
        assert!(!signers.is_empty(), "must have at least one signer");
        env.storage().instance().set(&DataKey::Signers, &signers);
    }

    /// Submit a proposal. Returns proposal ID. Each nonce must be unique.
    // #[sanctify(auth = "proposer", require_nonce_check)]
    pub fn submit(env: Env, proposer: Address, target: Address, nonce: u64) -> BytesN<32> {
        proposer.require_auth(); // SECURITY: auth first — #[sanctify(auth_gaps)]
        Self::require_signer(&env, &proposer);

        // SECURITY: replay protection — each nonce usable exactly once
        assert!(
            !env.storage().persistent().has(&DataKey::Nonce(nonce)),
            "replay: nonce already used"  // #[sanctify(require_nonce_check)]
        );
        env.storage().persistent().set(&DataKey::Nonce(nonce), &true);

        let proposal_id = Self::compute_id(&env, nonce);
        let proposal = Proposal {
            target,
            proposer,
            created_at: env.ledger().timestamp(),
            nonce,
        };
        env.storage().persistent().set(&DataKey::Proposal(proposal_id.clone()), &proposal);
        env.storage().persistent().set(&DataKey::ApprovalCount(proposal_id.clone()), &0_u32);

        // SECURITY: set timelock at submission — cannot be shortened later
        let unlock_at = env.ledger().timestamp() + TIMELOCK_SECONDS;
        env.storage().persistent().set(
            &DataKey::UnlockTime(proposal_id.clone()), &unlock_at // #[sanctify(require_timelock)]
        );
        proposal_id
    }

    /// Approve a proposal. Each signer may only approve once.
    // #[sanctify(auth = "signer")]
    pub fn approve(env: Env, signer: Address, proposal_id: BytesN<32>) {
        signer.require_auth(); // #[sanctify(auth_gaps)]
        Self::require_signer(&env, &signer);
        let signed_key = DataKey::HasSigned(proposal_id.clone(), signer.clone());
        // SECURITY: prevent double-voting by same signer
        assert!(!env.storage().persistent().has(&signed_key), "already approved");
        env.storage().persistent().set(&signed_key, &true);
        let count: u32 = env.storage().persistent()
            .get(&DataKey::ApprovalCount(proposal_id.clone())).unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::ApprovalCount(proposal_id),
            &count.checked_add(1).expect("overflow"),
        );
    }

    /// Execute a proposal after timelock and once threshold is reached.
    // #[sanctify(require_timelock, require_nonce_check)]
    pub fn execute(env: Env, executor: Address, proposal_id: BytesN<32>) {
        executor.require_auth(); // #[sanctify(auth_gaps)]
        Self::require_signer(&env, &executor);

        // SECURITY: timelock — revert if called too early
        let unlock_at: u64 = env.storage().persistent()
            .get(&DataKey::UnlockTime(proposal_id.clone()))
            .expect("unknown proposal"); // #[sanctify(require_timelock)]
        assert!(env.ledger().timestamp() >= unlock_at, "timelock: too early");

        // SECURITY: threshold — require M approvals
        let count: u32 = env.storage().persistent()
            .get(&DataKey::ApprovalCount(proposal_id.clone())).unwrap_or(0);
        assert!(count >= THRESHOLD, "threshold not reached"); // #[sanctify(invariants)]

        // SECURITY: mark executed BEFORE invoking target to prevent re-entrancy
        assert!(
            !env.storage().persistent().has(&DataKey::ExecutedAt(proposal_id.clone())),
            "already executed"
        );
        env.storage().persistent().set(
            &DataKey::ExecutedAt(proposal_id),
            &env.ledger().timestamp(),
        );
        // TODO: env.invoke_contract(&proposal.target, &Symbol::new(&env, "exec"), args);
    }

    fn require_signer(env: &Env, addr: &Address) {
        let signers: Vec<Address> = env.storage().instance()
            .get(&DataKey::Signers).expect("not initialised");
        assert!(signers.contains(addr), "not a registered signer");
    }

    fn compute_id(env: &Env, nonce: u64) -> BytesN<32> {
        // Deterministic proposal ID from nonce. Replace with full hash in production.
        let mut raw = [0u8; 32];
        raw[..8].copy_from_slice(&nonce.to_be_bytes());
        BytesN::from_array(env, &raw)
    }
}
"##
    
    fn multisig_config() -> &'static str {
        r#"# Generated by: sanctifier init --template multisig
# M-of-N governance security profile — strict mode enabled
ignore_paths          = ["target", ".git"]
enabled_rules         = ["auth_gaps", "panics", "arithmetic", "ledger_size", "invariants"]
ledger_limit          = 64000
strict_mode           = true
approaching_threshold = 0.8

[[custom_rules]]
name     = "require_nonce_check"
pattern  = "nonce"
severity = "error"

[[custom_rules]]
name     = "require_timelock"
pattern  = "TIMELOCK_SECONDS"
severity = "error"

[[custom_rules]]
name     = "no_unsafe_block"
pattern  = "unsafe"
severity = "error"
"#
    
    pub fn scaffold(template: &Template, output: &Path, force: bool) -> anyhow::Result<Vec<PathBuf>> {
        fs::create_dir_all(output.join("src"))?;

        let (contract_code, config_code, template_name) = match template {
            Template::Token    => (Self::token_contract(),    Self::token_config(),    "token"),
            Template::Amm      => (Self::amm_contract(),      Self::amm_config(),      "amm"),
            Template::Multisig => (Self::multisig_contract(), Self::multisig_config(), "multisig"),
        };

        let lib_rs      = output.join("src").join("lib.rs");
        let toml_path   = output.join(".sanctify.toml");
        let cargo_path  = output.join("Cargo.toml");

        Self::write_file(&lib_rs,     contract_code,                force)?;
        Self::write_file(&toml_path,  config_code,                  force)?;
        Self::write_file(&cargo_path, &Self::cargo_toml(template_name), force)?;

        Ok(vec![lib_rs, toml_path, cargo_path])
    }
}

pub fn exec(args: InitArgs, path: Option<PathBuf>) -> anyhow::Result<()> {
    use std::env;

    let target_dir = match path.or_else(|| args.output.clone()) {
        Some(p) => p,
        None    => env::current_dir()?,
    };

    if let Some(ref template) = args.template {
        // ── template scaffold path ──────────────────────────────────────
        if FileWriter::config_exists(&target_dir) && !args.force {
            OutputFormatter::display_existing_file_warning();
            anyhow::bail!("files already exist in output directory");
        }
        match TemplateGenerator::scaffold(template, &target_dir, args.force) {
            Ok(files) => {
                let name = match template {
                    Template::Token    => "token",
                    Template::Amm      => "amm",
                    Template::Multisig => "multisig",
                };
                OutputFormatter::display_scaffold_success(name, &target_dir);
                let refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
                OutputFormatter::display_scaffold_files(&refs);
                Ok(())
            }
            Err(e) => {
                OutputFormatter::display_error(&e);
                Err(e)
            }
        }
    } else {
        // ── default config-only path ────────────────────────────────────
        if FileWriter::config_exists(&target_dir) && !args.force {
            OutputFormatter::display_existing_file_warning();
            anyhow::bail!("configuration file already exists");
        }
        let config = ConfigGenerator::generate_default_config();
        match FileWriter::write_config(&config, &target_dir) {
            Ok(config_path) => {
                OutputFormatter::display_success(&config_path);
                OutputFormatter::display_template_hint();
                Ok(())
            }
            Err(e) => {
                OutputFormatter::display_error(&e);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_generate_default_config() {
        let config = ConfigGenerator::generate_default_config();
        assert_eq!(config.ignore_paths, vec!["target", ".git"]);
        assert_eq!(config.enabled_rules, vec!["auth_gaps", "panics", "arithmetic", "ledger_size"]);
        assert_eq!(config.ledger_limit, 64000);
        assert!(!config.strict_mode);
        assert_eq!(config.approaching_threshold, 0.8);
        assert_eq!(config.custom_rules.len(), 2);
        let rule1 = &config.custom_rules[0];
        assert_eq!(rule1.name, "no_unsafe_block");
        let rule2 = &config.custom_rules[1];
        assert_eq!(rule2.name, "no_mem_forget");
        assert_eq!(rule2.pattern, "std::mem::forget");
    }

    #[test]
    fn test_config_has_all_required_fields() {
        let config = ConfigGenerator::generate_default_config();
        assert!(!config.ignore_paths.is_empty(), "ignore_paths should not be empty");
        assert!(!config.enabled_rules.is_empty(), "enabled_rules should not be empty");
        assert!(config.ledger_limit > 0, "ledger_limit should be positive");
        assert!(config.approaching_threshold > 0.0 && config.approaching_threshold < 1.0);
    }

    #[test]
    fn test_custom_rules_have_valid_patterns() {
        let config = ConfigGenerator::generate_default_config();
        for rule in &config.custom_rules {
            assert!(!rule.name.is_empty(), "Custom rule name should not be empty");
            assert!(!rule.pattern.is_empty(), "Custom rule pattern should not be empty");
            let regex_result = regex::Regex::new(&rule.pattern);
            assert!(regex_result.is_ok(), "Pattern '{}' should be a valid regex", rule.pattern);
        }
    }

    #[test]
    fn test_config_exists_returns_false_when_no_file() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!FileWriter::config_exists(temp_dir.path()));
    }

    #[test]
    fn test_config_exists_returns_true_when_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".sanctify.toml"), "test").unwrap();
        assert!(FileWriter::config_exists(temp_dir.path()));
    }

    #[test]
    fn test_write_config_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = ConfigGenerator::generate_default_config();
        let result = FileWriter::write_config(&config, temp_dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_write_config_creates_valid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config = ConfigGenerator::generate_default_config();
        let result = FileWriter::write_config(&config, temp_dir.path());
        assert!(result.is_ok());
        let content = fs::read_to_string(result.unwrap()).unwrap();
        let parsed: Result<SanctifyConfig, _> = toml::from_str(&content);
        assert!(parsed.is_ok(), "Generated TOML should be parseable");
    }

    #[test]
    fn test_write_config_returns_correct_path() {
        let temp_dir = TempDir::new().unwrap();
        let config = ConfigGenerator::generate_default_config();
        let returned_path = FileWriter::write_config(&config, temp_dir.path()).unwrap();
        assert_eq!(returned_path, temp_dir.path().join(".sanctify.toml"));
    }

    #[test]
    fn test_exec_creates_config_in_temp_dir() {
        let temp_dir = TempDir::new().unwrap();
        let args = InitArgs { force: false, template: None, output: None };
        let result = exec(args, Some(temp_dir.path().to_path_buf()));
        assert!(result.is_ok(), "exec should succeed in empty directory");
        let config_path = temp_dir.path().join(".sanctify.toml");
        assert!(config_path.exists(), "Config file should be created");
        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: Result<SanctifyConfig, _> = toml::from_str(&content);
        assert!(parsed.is_ok(), "Generated TOML should be parseable");
    }

    #[test]
    fn test_exec_with_existing_file_without_force() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".sanctify.toml"), "existing content").unwrap();
        let args = InitArgs { force: false, template: None, output: None };
        let result = exec(args, Some(temp_dir.path().to_path_buf()));
        assert!(result.is_err(), "exec should fail without --force");
    }

    #[test]
    fn test_exec_with_force_overwrites_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".sanctify.toml"), "existing content").unwrap();
        let args = InitArgs { force: true, template: None, output: None };
        let result = exec(args, Some(temp_dir.path().to_path_buf()));
        assert!(result.is_ok(), "exec should succeed with force flag");
        let content = fs::read_to_string(temp_dir.path().join(".sanctify.toml")).unwrap();
        assert_ne!(content, "existing content", "File should be overwritten");
        assert!(content.contains("ignore_paths"), "Should contain default config");
    
    #[test]
    fn test_template_enum_variants_exist() {
        // Ensure all three variants compile and are distinct
        let t1 = Template::Token;
        let t2 = Template::Amm;
        let t3 = Template::Multisig;
        assert_ne!(t1, t2);
        assert_ne!(t2, t3);
        assert_ne!(t1, t3);
    
    #[test]
    fn test_token_template_creates_lib_rs_and_toml() {
        let temp_dir = TempDir::new().unwrap();
        let files = TemplateGenerator::scaffold(&Template::Token, temp_dir.path(), false).unwrap();
        let lib_rs    = temp_dir.path().join("src").join("lib.rs");
        let toml_path = temp_dir.path().join(".sanctify.toml");
        assert!(lib_rs.exists(),    "src/lib.rs should be created");
        assert!(toml_path.exists(), ".sanctify.toml should be created");
        assert_eq!(files.len(), 3, "scaffold should return 3 file paths");
        // Token contract must mention require_auth and checked_add
        let lib_content = fs::read_to_string(&lib_rs).unwrap();
        assert!(lib_content.contains("require_auth"), "token contract must call require_auth");
        assert!(lib_content.contains("checked_add"),  "token contract must use checked arithmetic");
        assert!(lib_content.contains("MAX_SUPPLY"),   "token contract must enforce supply cap");
    
    #[test]
    fn test_amm_template_strict_mode_false() {
        let temp_dir = TempDir::new().unwrap();
        TemplateGenerator::scaffold(&Template::Amm, temp_dir.path(), false).unwrap();
        let toml_content = fs::read_to_string(temp_dir.path().join(".sanctify.toml")).unwrap();
        // AMM profile uses strict_mode = false (less aggressive than token)
        assert!(toml_content.contains("strict_mode           = false"),
            "AMM config should not use strict mode");
        assert!(toml_content.contains("require_slippage_guard"),
            "AMM config must include slippage guard rule");
    
    #[test]
    fn test_amm_contract_includes_k_invariant() {
        let temp_dir = TempDir::new().unwrap();
        TemplateGenerator::scaffold(&Template::Amm, temp_dir.path(), false).unwrap();
        let lib_content = fs::read_to_string(temp_dir.path().join("src").join("lib.rs")).unwrap();
        assert!(lib_content.contains("K-invariant"),    "AMM contract must enforce K-invariant");
        assert!(lib_content.contains("min_out"),        "AMM contract must have slippage guard");
        assert!(lib_content.contains("require_auth()"), "AMM contract must call require_auth");
    
    #[test]
    fn test_multisig_template_includes_nonce_rule() {
        let temp_dir = TempDir::new().unwrap();
        TemplateGenerator::scaffold(&Template::Multisig, temp_dir.path(), false).unwrap();
        let toml_content = fs::read_to_string(temp_dir.path().join(".sanctify.toml")).unwrap();
        assert!(toml_content.contains("require_nonce_check"), "multisig config must include nonce rule");
        assert!(toml_content.contains("require_timelock"),    "multisig config must include timelock rule");
        assert!(toml_content.contains("strict_mode           = true"), "multisig config must use strict mode");
    }
}
