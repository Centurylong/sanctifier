//! Sanctifier language server.
//!
//! Real-time Soroban contract analysis in the editor: diagnostics as you type
//! and on save, and hover documentation explaining each finding.
//!
//! The server speaks LSP over stdio and is synchronous by design — see
//! `Cargo.toml` for why there is no async runtime here.

pub mod analysis;
pub mod protocol;
pub mod server;

use std::io::{self, BufReader};

/// Run the language server over stdin/stdout until the client disconnects.
pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    server::serve(&mut reader, &mut writer)
}
