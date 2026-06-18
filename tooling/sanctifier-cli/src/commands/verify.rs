use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct VerifyArgs {
    /// Path to a contract directory, workspace directory, or a single .rs file.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Exit with a non-zero status code if any invariant cannot be proven
    /// (Refuted or Unknown). Useful in CI.
    #[arg(long, default_value_t = false)]
    pub strict: bool,

    /// Emit results as JSON instead of human-readable text.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

pub fn exec(_args: VerifyArgs) -> anyhow::Result<()> {
    // Stub — implemented in subsequent commits.
    Ok(())
}
