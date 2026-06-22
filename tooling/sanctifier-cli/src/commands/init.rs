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
