use clap::Args;
use sanctifier_core::{invariant::InvariantDecl, Analyzer, SanctifyConfig};
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

/// Scan `path` (file or directory) and return all invariant declarations found.
pub(crate) fn discover_invariants(path: &Path) -> Vec<InvariantDecl> {
    let config = SanctifyConfig::default();
    let analyzer = Analyzer::new(config.clone());

    let mut rs_files: Vec<PathBuf> = Vec::new();
    if path.is_dir() {
        collect_rs_files(path, &config.ignore_paths, &mut rs_files);
    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        rs_files.push(path.to_path_buf());
    }

    let mut all_decls: Vec<InvariantDecl> = Vec::new();
    for file in rs_files {
        let source = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let label = file.display().to_string();
        let decls = analyzer.scan_invariant_attrs(&source, &label);
        all_decls.extend(decls);
    }
    all_decls
}

pub fn exec(_args: VerifyArgs) -> anyhow::Result<()> {
    // Stub — result dispatch and output added in next commits.
    Ok(())
}
