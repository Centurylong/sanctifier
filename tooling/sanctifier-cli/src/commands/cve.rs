use crate::vulndb::VulnDatabase;
use clap::{Args, Subcommand};
use colored::*;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;

#[derive(Args)]
pub struct CveArgs {
    #[command(subcommand)]
    pub command: CveCommand,
}

#[derive(Subcommand)]
pub enum CveCommand {
    /// Search the vulnerability database by keyword
    Search {
        /// Keyword to search (matches id, name, description, tags, category)
        #[arg(long, short)]
        keyword: String,

        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// List all vulnerabilities with optional filters
    List {
        /// Filter by category (e.g. access-control, arithmetic, storage)
        #[arg(long, short)]
        category: Option<String>,

        /// Filter by severity (critical, high, medium, low)
        #[arg(long, short)]
        severity: Option<String>,

        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Show full details for a specific vulnerability by ID
    Show {
        /// Vulnerability ID (e.g. SOL-2024-001 or SOB-2024-015)
        id: String,

        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Export the database as JSON or RSS
    Export {
        /// Output format: json or rss
        #[arg(long, default_value = "json")]
        format: String,

        /// Write output to this file instead of stdout
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Base URL used in RSS links (default: https://sanctifier.dev)
        #[arg(long, default_value = "https://sanctifier.dev")]
        base_url: String,
    },
    /// Start a local HTTP server exposing GET /api/vulndb
    Serve {
        /// Port to listen on
        #[arg(long, short, default_value = "7654")]
        port: u16,
    },
}

pub fn exec(args: CveArgs) -> anyhow::Result<()> {
    let db = VulnDatabase::load_default();

    match args.command {
        CveCommand::Search { keyword, format } => {
            let results = db.search(&keyword);
            if results.is_empty() {
                println!("{}", format!("No vulnerabilities matched '{}'.", keyword).yellow());
                return Ok(());
            }
            if format == "json" {
                let entries: Vec<_> = results.into_iter().collect();
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                println!(
                    "{} {} vulnerabilities matched '{}':\n",
                    "●".cyan(),
                    results.len(),
                    keyword.bold()
                );
                for v in &results {
                    print_vuln_summary(v);
                }
            }
        }

        CveCommand::List { category, severity, format } => {
            let filtered: Vec<_> = db
                .vulnerabilities
                .iter()
                .filter(|v| {
                    category
                        .as_deref()
                        .map(|c| v.category.to_lowercase() == c.to_lowercase())
                        .unwrap_or(true)
                        && severity
                            .as_deref()
                            .map(|s| v.severity.to_lowercase() == s.to_lowercase())
                            .unwrap_or(true)
                })
                .collect();

            if filtered.is_empty() {
                println!("{}", "No vulnerabilities matched the given filters.".yellow());
                return Ok(());
            }

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&filtered)?);
            } else {
                let cat_label = category
                    .as_deref()
                    .map(|c| format!(" [category: {}]", c))
                    .unwrap_or_default();
                let sev_label = severity
                    .as_deref()
                    .map(|s| format!(" [severity: {}]", s))
                    .unwrap_or_default();
                println!(
                    "{} {} entries{}{}\n",
                    "●".cyan(),
                    filtered.len(),
                    cat_label,
                    sev_label
                );
                for v in &filtered {
                    print_vuln_summary(v);
                }
            }
        }

        CveCommand::Show { id, format } => match db.get_by_id(&id) {
            None => {
                eprintln!("{} Vulnerability '{}' not found.", "✗".red(), id);
                std::process::exit(1);
            }
            Some(v) => {
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&v)?);
                } else {
                    print_vuln_detail(v);
                }
            }
        },

        CveCommand::Export { format, output, base_url } => {
            let content = match format.as_str() {
                "rss" => db.to_rss(&base_url),
                _ => db.to_json()?,
            };
            match output {
                Some(path) => {
                    fs::write(&path, &content)?;
                    println!(
                        "{} Wrote {} ({} bytes) to {}",
                        "✓".green(),
                        format,
                        content.len(),
                        path.display()
                    );
                }
                None => print!("{}", content),
            }
        }

        CveCommand::Serve { port } => {
            serve_http(&db, port)?;
        }
    }

    Ok(())
}

fn severity_color(sev: &str) -> colored::ColoredString {
    match sev.to_lowercase().as_str() {
        "critical" => sev.to_uppercase().red().bold(),
        "high" => sev.to_uppercase().yellow().bold(),
        "medium" => sev.to_uppercase().cyan(),
        "low" => sev.to_uppercase().white(),
        _ => sev.to_uppercase().normal(),
    }
}

fn print_vuln_summary(v: &crate::vulndb::VulnEntry) {
    let cvss = v
        .cvss
        .map(|c| format!(" CVSS {:.1}", c))
        .unwrap_or_default();
    println!(
        "  {} {}  [{}{}]  {}",
        v.id.bold(),
        severity_color(&v.severity),
        v.category.dimmed(),
        cvss.dimmed(),
        v.name
    );
}

fn print_vuln_detail(v: &crate::vulndb::VulnEntry) {
    println!("\n{}", "─".repeat(60));
    println!("{} — {}", v.id.bold(), v.name.bold());
    println!("{}", "─".repeat(60));
    println!(
        "Severity : {}{}",
        severity_color(&v.severity),
        v.cvss
            .map(|c| format!("  (CVSS {:.1})", c))
            .unwrap_or_default()
    );
    println!("Category : {}", v.category);
    if let Some(av) = &v.affected_versions {
        println!("Affects  : {}", av);
    }
    println!("\n{}\n", v.description);

    if let Some(poc) = &v.poc_exploit {
        println!("{}", "PoC:".yellow().bold());
        for line in poc.lines() {
            println!("  {}", line);
        }
        println!();
    }

    if let Some(patch) = &v.patch {
        println!("{}", "Patch:".green().bold());
        for line in patch.lines() {
            println!("  {}", line);
        }
        println!();
    }

    println!("{}", "Recommendation:".cyan().bold());
    println!("  {}", v.recommendation);

    if !v.tags.is_empty() {
        println!("\nTags: {}", v.tags.join(", ").dimmed());
    }
    if !v.references.is_empty() {
        println!("\nReferences:");
        for r in &v.references {
            println!("  • {}", r);
        }
    }
    println!("{}\n", "─".repeat(60));
}

fn serve_http(db: &VulnDatabase, port: u16) -> anyhow::Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)?;
    println!(
        "{} Sanctifier CVE database listening on http://{}\n  GET /api/vulndb          → full JSON database\n  GET /api/vulndb/<id>     → single entry JSON\n  GET /api/vulndb/feed.rss → RSS feed\n\nPress Ctrl-C to stop.",
        "✓".green(),
        addr
    );

    let db_json = db.to_json()?;
    let db_rss = db.to_rss(&format!("http://{addr}"));

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let reader = BufReader::new(&stream);
        let request_line = match reader.lines().next() {
            Some(Ok(l)) => l,
            _ => continue,
        };

        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .to_string();

        let (status, content_type, body): (&str, &str, String) =
            if path == "/api/vulndb" || path == "/api/vulndb/" {
                ("200 OK", "application/json", db_json.clone())
            } else if path == "/api/vulndb/feed.rss" {
                ("200 OK", "application/rss+xml", db_rss.clone())
            } else if let Some(id) = path.strip_prefix("/api/vulndb/") {
                match db.get_by_id(id) {
                    Some(entry) => (
                        "200 OK",
                        "application/json",
                        serde_json::to_string_pretty(entry).unwrap_or_default(),
                    ),
                    None => (
                        "404 Not Found",
                        "application/json",
                        format!("{{\"error\":\"not found\",\"id\":\"{id}\"}}"),
                    ),
                }
            } else {
                (
                    "404 Not Found",
                    "text/plain",
                    "Not found. Try GET /api/vulndb".to_string(),
                )
            };

        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    }

    Ok(())
}
