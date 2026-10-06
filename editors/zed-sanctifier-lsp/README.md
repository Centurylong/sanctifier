# Sanctifier LSP for Zed

This extension connects Zed's Rust support to Sanctifier's existing Language Server Protocol server.

It does not bundle a second analyzer or language server. Zed locates the installed Sanctifier executable on the user's PATH and launches:

~~~sh
sanctifier lsp --stdio
~~~

That shared server publishes Sanctifier diagnostics for open Rust files and provides hover details for findings.

## Requirements

- Zed with Rust extension support.
- Sanctifier installed so the sanctifier executable is available on the environment Zed sees.
- A Rust or Soroban workspace.

Verify the executable before loading the extension:

~~~sh
sanctifier lsp --help
~~~

## Install as a development extension

1. Open Zed's Extensions view.
2. Choose **Install Dev Extension**.
3. Select this directory: editors/zed-sanctifier-lsp.
4. Open a Rust file in a project.

Zed starts the server for Rust buffers. Diagnostics appear through Zed's normal Problems and inline diagnostic UI, and finding details are available through LSP hover.

## Troubleshooting

### Sanctifier was not found on PATH

The extension uses Zed's worktree environment instead of reading the host environment directly. Start Zed from an environment where Sanctifier resolves, or otherwise make the executable available on the PATH Zed inherits.

### No diagnostics appear

Confirm the same source produces findings through the CLI, then confirm the LSP starts:

~~~sh
sanctifier analyze path/to/contract
sanctifier lsp --stdio
~~~

The LSP uses full-document synchronization and publishes diagnostics on open, change, and save. See tooling/sanctifier-lsp/README.md for the protocol surface and development checks.

## Development

Zed compiles Rust extensions to wasm32-wasip2. With the target installed, a focused compile check can be run from this directory:

~~~sh
cargo check --target wasm32-wasip2
~~~
