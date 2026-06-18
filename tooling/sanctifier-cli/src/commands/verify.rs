use clap::Args;
use std::path::{Path, PathBuf};

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

/// Recursively collect every `.rs` file under `dir`, skipping paths that
/// contain any segment in `ignore` (e.g. "target", ".git").
pub(crate) fn collect_rs_files(dir: &Path, ignore: &[String], out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if ignore.iter().any(|p| name.contains(p.as_str())) {
                continue;
            }
            collect_rs_files(&path, ignore, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

pub fn exec(_args: VerifyArgs) -> anyhow::Result<()> {
    // Stub — invariant discovery and dispatch added in next commits.
    Ok(())
}
