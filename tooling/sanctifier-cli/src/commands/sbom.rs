use clap::Args;
use colored::*;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct SbomArgs {
    /// Path to the crate or workspace to generate an SBOM for (must contain Cargo.lock)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct Component {
    #[serde(rename = "type")]
    component_type: &'static str,
    name: String,
    version: String,
    purl: String,
}

#[derive(Debug, Serialize)]
struct Sbom {
    #[serde(rename = "bomFormat")]
    bom_format: &'static str,
    #[serde(rename = "specVersion")]
    spec_version: &'static str,
    version: u32,
    #[serde(rename = "serialNumber")]
    serial_number: String,
    metadata: Metadata,
    components: Vec<Component>,
}

#[derive(Debug, Serialize)]
struct Metadata {
    timestamp: String,
    tools: Vec<Tool>,
}

#[derive(Debug, Serialize)]
struct Tool {
    vendor: &'static str,
    name: &'static str,
    version: &'static str,
}

/// Generates a minimal CycloneDX-1.5-compatible JSON SBOM by parsing every
/// `[[package]]` entry out of the target project's `Cargo.lock`. This deliberately
/// parses the lockfile directly (rather than shelling out to `cargo metadata`) so
/// it works even for a contract analyzed in isolation without a full build.
pub fn exec(args: SbomArgs) -> anyhow::Result<()> {
    let lock_path = args.path.join("Cargo.lock");
    let content = fs::read_to_string(&lock_path).map_err(|e| {
        anyhow::anyhow!("failed to read {:?}: {e} (is this a Rust project with a Cargo.lock?)", lock_path)
    })?;
    let doc: toml::Value = content.parse()?;

    let mut components = Vec::new();
    if let Some(packages) = doc.get("package").and_then(|p| p.as_array()) {
        for pkg in packages {
            let (Some(name), Some(version)) = (
                pkg.get("name").and_then(|n| n.as_str()),
                pkg.get("version").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            components.push(Component {
                component_type: "library",
                name: name.to_string(),
                version: version.to_string(),
                purl: format!("pkg:cargo/{name}@{version}"),
            });
        }
    }
    components.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));

    let sbom = Sbom {
        bom_format: "CycloneDX",
        spec_version: "1.5",
        version: 1,
        serial_number: format!("urn:uuid:{}", deterministic_uuid(&components)),
        metadata: Metadata {
            timestamp: chrono_now(),
            tools: vec![Tool {
                vendor: "Sanctifier",
                name: "sanctifier-cli",
                version: env!("CARGO_PKG_VERSION"),
            }],
        },
        components,
    };

    let json = serde_json::to_string_pretty(&sbom)?;

    match &args.output {
        Some(path) => {
            fs::write(path, &json)?;
            println!(
                "{} Wrote SBOM for {} component(s) to {:?}",
                "✅".green(),
                sbom.components.len(),
                path
            );
        }
        None => println!("{json}"),
    }

    Ok(())
}

/// A short, stable identifier derived from the component list content so the
/// same dependency set always produces the same serial number, without
/// depending on a `uuid` crate.
fn deterministic_uuid(components: &[Component]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    for c in components {
        c.name.hash(&mut hasher);
        c.version.hash(&mut hasher);
    }
    let h = hasher.finish();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (h >> 32) as u32,
        (h >> 16) & 0xffff,
        h & 0xfff,
        (h >> 48) & 0xffff,
        h & 0xffff_ffff_ffff
    )
}

/// Formats the current time as an RFC 3339 UTC timestamp using a small,
/// dependency-free civil-calendar conversion (Howard Hinnant's algorithm),
/// since this crate has no date/time crate on hand.
fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);
    let (hour, minute, second) = (time_of_day / 3600, (time_of_day / 60) % 60, time_of_day % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}
