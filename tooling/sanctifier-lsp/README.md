# sanctifier-lsp

Language Server Protocol server for Sanctifier — Soroban contract findings
reported in the editor, as you type, instead of after you finish.

## Running

```sh
sanctifier lsp --stdio        # via the CLI
sanctifier-lsp --stdio        # or the standalone binary
```

`--stdio` is required rather than assumed. Editors always pass it, and a bare
invocation would otherwise sit silently blocked on stdin in front of a human
who typed the command by hand.

## What it implements

| LSP method | Behaviour |
|---|---|
| `initialize` | Advertises hover and full-document text sync |
| `textDocument/didOpen` / `didChange` / `didSave` | Re-analyses and publishes diagnostics |
| `textDocument/didClose` | Forgets the document |
| `textDocument/hover` | Markdown explaining the findings on the hovered line |
| `shutdown` / `exit` | Clean teardown |

Unknown requests are answered with `MethodNotFound` rather than ignored — a
silent server leaves the client blocked forever on a reply that never comes.

Severity maps onto the protocol's four levels: `critical` and `high` become
Error, `medium` becomes Warning, `low` and `info` become Information. Nothing
maps to Hint, which most editors render as a nearly invisible dotted underline
and would effectively hide findings the engine did choose to report.

## Design notes

**No async runtime.** LSP over stdio is a request/response loop on two pipes.
`tower-lsp` would pull in tokio and its dependency tree, adding build time to
every editor install, for concurrency this server does not have. The protocol
layer here is a couple of hundred lines and unit-testable without a runtime.

**Full text sync, not incremental.** The analyzer takes whole source text, so
applying incremental edits would only mean reassembling the document before
handing it over intact. Full sync is more bytes on the wire and strictly less
code that can be wrong. Measured cost of re-analysing from scratch on an
89 KB file: **~44 ms**, against the 500 ms budget in issue #138.

**Stateless analysis.** Every detector is a pure function of the source text,
so there is no cache to invalidate and no way to serve a stale result — which
is what an incremental cache would risk for milliseconds the editor cannot
perceive.

**Client content is the source of truth.** Documents are held in memory from
the last sync rather than re-read from disk. Analysing the file on disk would
report diagnostics for code the developer has already changed.

**Findings without a line still surface.** Some detectors report position as a
`"function:line"` context string and some report nothing at all. The line is
parsed out of the former; the latter are pinned to line 1 rather than dropped,
because an editor showing fewer problems than `sanctifier analyze` is worse
than one showing a finding in an imprecise place.

## Development

```sh
cargo test                                   # 32 unit tests
cargo clippy --all-targets -- -D warnings
cargo build --release
python3 ../../scripts/lsp-conformance.py ./target/release/sanctifier-lsp
```

The unit tests drive the server over in-memory buffers. The conformance script
drives the real binary over real pipes, which is the only place a framing or
flushing bug shows up — an in-memory `Cursor` never blocks, so a missing flush
or a miscounted `Content-Length` looks fine there and hangs a real editor. It
also measures the edit-to-diagnostics latency.

## Editor integration

### VS Code

The extension in [`editors/vscode/`](../../editors/vscode/) is not yet an LSP
client. To use the server today, point any generic LSP bridge at
`sanctifier lsp --stdio` for `rust` documents. Wiring
`vscode-languageclient` into the extension and publishing to the marketplace
is deliberately left out of this change — see the PR description.

### Neovim (built-in client)

```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "rust",
  callback = function()
    vim.lsp.start({
      name = "sanctifier",
      cmd = { "sanctifier", "lsp", "--stdio" },
      root_dir = vim.fs.dirname(vim.fs.find({ "Cargo.toml" }, { upward = true })[1]),
    })
  end,
})
```

### IntelliJ

Skeleton at [`editors/intellij/`](../../editors/intellij/).
