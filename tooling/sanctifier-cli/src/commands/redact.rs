use clap::Args;
use colored::*;
use regex::Regex;
use std::fs;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct RedactArgs {
    /// Path to a report/finding file to redact (e.g. `sanctifier analyze --format json` output,
    /// or a `.sanctify-baseline.json`)
    pub input: PathBuf,

    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Scrubs values that shouldn't leave the local machine from a report before
/// it's shared or uploaded: Stellar public keys, secret seeds, contract
/// addresses, and absolute paths under the user's home directory. Sanctifier
/// itself never sends scan data anywhere on its own (`analyze` runs entirely
/// offline; only `update`/`--webhook-url` touch the network, and only when
/// explicitly configured) — this command exists for the separate case of a
/// human sharing a saved report externally.
pub fn exec(args: RedactArgs) -> anyhow::Result<()> {
    let content = fs::read_to_string(&args.input)?;
    let redacted = redact_text(&content);

    match &args.output {
        Some(path) => {
            fs::write(path, &redacted)?;
            println!("{} Wrote redacted report to {:?}", "✅".green(), path);
        }
        None => print!("{redacted}"),
    }

    Ok(())
}

pub fn redact_text(input: &str) -> String {
    // Stellar StrKey-encoded values: G (account), S (seed), C (contract),
    // all 56 base32 characters.
    let strkey = Regex::new(r"\b[GSC][A-Z2-7]{55}\b").unwrap();
    let mut out = strkey.replace_all(input, "[REDACTED-STELLAR-KEY]").into_owned();

    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            out = out.replace(&home, "~");
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // A valid StrKey body is exactly 55 chars from the RFC4648 base32 alphabet
    // (A-Z, 2-7); anything shorter/longer or containing 0/1/8/9 won't match.
    const BODY: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVW";

    #[test]
    fn redacts_stellar_public_key() {
        let key = format!("G{BODY}");
        let input = format!("admin: {key}");
        let out = redact_text(&input);
        assert!(!out.contains(&key));
        assert!(out.contains("[REDACTED-STELLAR-KEY]"));
    }

    #[test]
    fn redacts_secret_seed() {
        let key = format!("S{BODY}");
        let input = format!("seed: {key}");
        let out = redact_text(&input);
        assert!(out.contains("[REDACTED-STELLAR-KEY]"));
    }

    #[test]
    fn leaves_unrelated_text_untouched() {
        let input = "no secrets here, just findings: 3 high severity issues";
        assert_eq!(redact_text(input), input);
    }
}
