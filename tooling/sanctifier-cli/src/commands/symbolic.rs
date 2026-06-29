use clap::Args;
use colored::*;
use sanctifier_core::Analyzer;
use sanctifier_core::SanctifyConfig;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct SymbolicArgs {
    /// Path to the contract directory or file
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

pub fn exec(args: SymbolicArgs) -> anyhow::Result<()> {
    let path = &args.path;

    println!(
        "{} Running symbolic execution (Path-enumeration prototype) on {:?}",
        "🔍".blue(),
        path
    );
    
    let config = SanctifyConfig::default();
    let analyzer = Analyzer::new(config);

    let mut all_issues = Vec::new();

    if path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(content) = fs::read_to_string(path) {
                let issues = analyzer.scan_symbolic_paths(&content);
                for mut issue in issues {
                    issue.location = format!("{}:{}", path.display(), issue.location);
                    all_issues.push(issue);
                }
            }
        }
    } else if path.is_dir() {
        let mut stack = vec![path.to_path_buf()];
        while let Some(current) = stack.pop() {
            if let Ok(entries) = fs::read_dir(current) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                        if let Ok(content) = fs::read_to_string(&p) {
                            let issues = analyzer.scan_symbolic_paths(&content);
                            for mut issue in issues {
                                issue.location = format!("{}:{}", p.display(), issue.location);
                                all_issues.push(issue);
                            }
                        }
                    }
                }
            }
        }
    }

    if all_issues.is_empty() {
        println!("{} No always-revert paths or unreachable branches found.", "✅".green());
    } else {
        println!("\n{} Found Symbolic Execution Issues!", "⚠️".yellow());
        for issue in all_issues {
            println!(
                "   {} Function: {}",
                "->".red(),
                issue.function_name.bold()
            );
            println!("      Location: {}", issue.location);
            println!("      Message: {}", issue.description);
        }
    }

    println!("\nLimitations: This is a bounded AST-level path enumeration prototype.");
    println!("It currently evaluates if/else branches and detects explicit panics/asserts/unwraps.");
    println!("It does not yet resolve variable states or inter-procedural calls fully.\n");

    Ok(())
}
