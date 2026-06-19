//! Structured logging via tracing.
//! - Results go to stdout
//! - Logs/diagnostics go to stderr
//! - Controlled by -v/-vv, --quiet, and env-filter

use tracing::Level;
use tracing_subscriber::EnvFilter;

/// Initialize the tracing subscriber for structured logging.
///
/// - `verbose`: 0 = default (warn+), 1 = info+, 2 = debug+
/// - `quiet`: if true, suppresses all logging (only errors)
pub fn init_logging(verbose: u8, quiet: bool) {
    let filter = if quiet {
        EnvFilter::new("error")
    } else {
        match verbose {
            0 => EnvFilter::new("warn"),
            1 => EnvFilter::new("info"),
            _ => EnvFilter::new("debug"),
        }
    };

    // Also respect RUST_LOG env var if set
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or(filter);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(true)
        .with_writer(std::io::stderr) // All logs go to stderr
        .init();
}