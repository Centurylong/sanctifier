use crate::commands::analyze::AnalyzeArgs;
use clap::Args;
use colored::*;
use notify::{recommended_watcher, EventKind, RecursiveMode, Watcher};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Arguments for `sanctifier watch`.
#[derive(Args)]
pub struct WatchArgs {
    /// Path to a contract directory, workspace, or single `.rs` file to watch
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,

    /// Debounce window in milliseconds before re-running after a change
    #[arg(short, long, default_value = "300")]
    pub debounce: u64,

    /// Output format passed through to `analyze` (text | json)
    #[arg(short, long, default_value = "text")]
    pub format: String,
}

type WatchEvent = notify::Result<notify::Event>;

/// Returns true when an event represents a created/modified/removed `.rs` file.
/// Access (read) events and non-Rust paths are ignored so the watcher only
/// reacts to actual source edits.
fn is_rs_event(event: &notify::Event) -> bool {
    let is_mutation = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    );
    is_mutation
        && event
            .paths
            .iter()
            .any(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
}

/// Run `sanctifier watch`: re-run analysis whenever a `.rs` file under `path`
/// changes, debounced, until interrupted with Ctrl-C.
pub fn exec(args: WatchArgs) -> anyhow::Result<()> {
    let analyze_args = watch_child_args(&args);
    exec_loop(args.path, args.debounce, analyze_args)
}

/// Run `sanctifier analyze --watch` through the same watcher while preserving
/// the one-shot analyze options for each child invocation.
pub fn exec_analyze(args: AnalyzeArgs) -> anyhow::Result<()> {
    let path = watch_root(&args.path);
    let analyze_args = analyze_child_args(&args);
    exec_loop(path, 300, analyze_args)
}

fn watch_root(path: &Path) -> PathBuf {
    if path.is_file() {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    } else {
        path.to_path_buf()
    }
}

fn exec_loop(path: PathBuf, debounce_ms: u64, analyze_args: Vec<OsString>) -> anyhow::Result<()> {
    if !path.exists() {
        anyhow::bail!("path {:?} does not exist", path);
    }

    let (tx, rx) = channel::<WatchEvent>();
    let mut watcher = recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    watcher.watch(&path, RecursiveMode::Recursive)?;

    // Ctrl-C flips this flag; the loop polls it so shutdown is cooperative and
    // the watcher/Drop runs cleanly instead of the process being hard-killed.
    let shutdown = Arc::new(AtomicBool::new(false));
    {
        let shutdown = Arc::clone(&shutdown);
        ctrlc::set_handler(move || shutdown.store(true, Ordering::SeqCst))
            .map_err(|e| anyhow::anyhow!("failed to install Ctrl-C handler: {e}"))?;
    }

    // Show results immediately, before the first change.
    run_analysis(&analyze_args);
    print_watching(&path);

    while !shutdown.load(Ordering::SeqCst) {
        // Poll so Ctrl-C is observed even while idle.
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) if is_rs_event(&event) => {
                debounce(&rx, debounce_ms, &shutdown);
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                run_analysis(&analyze_args);
                print_watching(&path);
            }
            Ok(_) => {} // unrelated event or a watch error — ignore
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    eprintln!("\n{} Watch stopped.", "✓".green());
    Ok(())
}

/// Wait until source files have been quiet for `debounce_ms`. Each new `.rs`
/// change resets the timer, so a burst of saves coalesces into a single run and
/// a change arriving mid-window cancels the pending run.
fn debounce(rx: &Receiver<WatchEvent>, debounce_ms: u64, shutdown: &AtomicBool) {
    let window = Duration::from_millis(debounce_ms);
    let mut deadline = Instant::now() + window;

    loop {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        match rx.recv_timeout(remaining) {
            Ok(Ok(event)) if is_rs_event(&event) => {
                deadline = Instant::now() + window;
            }
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Clear the screen and re-run `sanctifier analyze` as a child process.
///
/// Running analysis in a subprocess keeps the watcher alive: `analyze` calls
/// `std::process::exit` on findings / invalid projects, which would otherwise
/// terminate the whole watch session.
fn run_analysis(analyze_args: &[OsString]) {
    clear_screen();

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("{} could not locate sanctifier binary: {e}", "❌".red());
            return;
        }
    };

    let status = Command::new(exe).args(analyze_args).status();

    if let Err(e) = status {
        eprintln!("{} failed to run analysis: {e}", "❌".red());
    }
}

fn watch_child_args(args: &WatchArgs) -> Vec<OsString> {
    vec![
        OsString::from("analyze"),
        args.path.as_os_str().to_os_string(),
        OsString::from("--format"),
        OsString::from(args.format.as_str()),
    ]
}

fn analyze_child_args(args: &AnalyzeArgs) -> Vec<OsString> {
    let mut child = vec![
        OsString::from("analyze"),
        args.path.as_os_str().to_os_string(),
        OsString::from("--format"),
        OsString::from(args.format.as_str()),
        OsString::from("--limit"),
        OsString::from(args.limit.to_string()),
    ];

    if let Some(vuln_db) = &args.vuln_db {
        child.push(OsString::from("--vuln-db"));
        child.push(vuln_db.as_os_str().to_os_string());
    }
    for webhook_url in &args.webhook_urls {
        child.push(OsString::from("--webhook-url"));
        child.push(OsString::from(webhook_url.as_str()));
    }
    if args.no_baseline {
        child.push(OsString::from("--no-baseline"));
    }
    if args.profile {
        child.push(OsString::from("--profile"));
    }
    if let Some(max_memory) = args.max_memory {
        child.push(OsString::from("--max-memory"));
        child.push(OsString::from(max_memory.to_string()));
    }

    child
}

fn clear_screen() {
    // ANSI: clear screen + move cursor to home.
    eprint!("\x1B[2J\x1B[1;1H");
    let _ = std::io::stderr().flush();
}

fn print_watching(path: &Path) {
    eprintln!(
        "\n👀 Watching {} for .rs changes — press Ctrl-C to stop.",
        path.display().to_string().cyan()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, Event, EventKind, ModifyKind};

    #[test]
    fn detects_rs_modifications() {
        let event =
            Event::new(EventKind::Modify(ModifyKind::Any)).add_path(PathBuf::from("lib.rs"));
        assert!(is_rs_event(&event));
    }

    #[test]
    fn detects_rs_creation_and_removal() {
        let created = Event::new(EventKind::Create(notify::event::CreateKind::File))
            .add_path("new.rs".into());
        let removed = Event::new(EventKind::Remove(notify::event::RemoveKind::File))
            .add_path("old.rs".into());
        assert!(is_rs_event(&created));
        assert!(is_rs_event(&removed));
    }

    #[test]
    fn ignores_non_rs_files() {
        let event = Event::new(EventKind::Modify(ModifyKind::Any)).add_path("notes.txt".into());
        assert!(!is_rs_event(&event));
    }

    #[test]
    fn ignores_access_events() {
        let event = Event::new(EventKind::Access(AccessKind::Read)).add_path("lib.rs".into());
        assert!(!is_rs_event(&event));
    }

    #[test]
    fn watch_child_uses_positional_analyze_path() {
        let args = WatchArgs {
            path: PathBuf::from("contract"),
            debounce: 300,
            format: "json".to_string(),
        };
        let rendered: Vec<String> = watch_child_args(&args)
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            rendered,
            vec![
                "analyze".to_string(),
                "contract".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ]
        );
        assert!(!rendered.iter().any(|arg| arg == "--path"));
    }

    #[test]
    fn cargo_manifest_watch_uses_parent_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manifest = temp_dir.path().join("Cargo.toml");
        std::fs::write(&manifest, "[package]\nname = \"watch-root\"\nversion = \"0.1.0\"\n").unwrap();

        assert_eq!(watch_root(&manifest), temp_dir.path());
        assert_eq!(watch_root(temp_dir.path()), temp_dir.path());
    }

    #[test]
    fn relative_file_watch_uses_current_directory() {
        let manifest = Path::new("Cargo.toml");
        assert!(manifest.is_file(), "the CLI crate manifest is present");
        assert_eq!(watch_root(manifest), PathBuf::from("."));

        let nested_file = Path::new("src/main.rs");
        assert!(nested_file.is_file(), "the CLI source file is present");
        assert_eq!(watch_root(nested_file), PathBuf::from("src"));
    }

    #[test]
    fn analyze_watch_child_preserves_options_without_recursing() {
        let args = AnalyzeArgs {
            path: PathBuf::from("contract"),
            format: "json".to_string(),
            limit: 8192,
            vuln_db: Some(PathBuf::from("vulns.json")),
            webhook_urls: vec!["https://example.invalid/hook".to_string()],
            no_baseline: true,
            profile: true,
            max_memory: Some(512),
            watch: true,
        };
        let rendered: Vec<String> = analyze_child_args(&args)
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(rendered[0], "analyze");
        assert_eq!(rendered[1], "contract");
        assert!(!rendered.iter().any(|arg| arg == "--watch"));
        assert!(!rendered.iter().any(|arg| arg == "--path"));
        for expected in [
            "--format",
            "json",
            "--limit",
            "8192",
            "--vuln-db",
            "vulns.json",
            "--webhook-url",
            "https://example.invalid/hook",
            "--no-baseline",
            "--profile",
            "--max-memory",
            "512",
        ] {
            assert!(rendered.iter().any(|arg| arg == expected), "missing {expected}");
        }
    }
}
