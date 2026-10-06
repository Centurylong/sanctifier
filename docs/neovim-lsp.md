# Neovim LSP integration

Sanctifier can publish diagnostics and hover information directly in Neovim through its built-in Language Server Protocol client.

## Requirements

- Neovim 0.11.3 or newer.
- A current `nvim-lspconfig` installation.
- The `sanctifier` executable on Neovim's `$PATH`.

Install Sanctifier with one of the supported methods from the project README. For example:

```bash
cargo install --git https://github.com/Centurylong/sanctifier sanctifier-cli
sanctifier lsp --help
```

The language server uses stdio; the command Neovim starts is `sanctifier lsp --stdio`.

## nvim-lspconfig recipe

Current nvim-lspconfig releases use Neovim's `vim.lsp.config` / `vim.lsp.enable` API. The older `require('lspconfig').<server>.setup{}` interface is deprecated.

Add this to `init.lua` after nvim-lspconfig is available:

```lua
vim.lsp.config('sanctifier', {
  cmd = { 'sanctifier', 'lsp', '--stdio' },
  filetypes = { 'rust' },
  root_markers = { 'Cargo.toml', '.git' },
})

vim.lsp.enable('sanctifier')
```

Alternatively, place the configuration in `~/.config/nvim/after/lsp/sanctifier.lua`:

```lua
return {
  cmd = { 'sanctifier', 'lsp', '--stdio' },
  filetypes = { 'rust' },
  root_markers = { 'Cargo.toml', '.git' },
}
```

and keep only this in `init.lua`:

```lua
vim.lsp.enable('sanctifier')
```

Open a Rust file inside a Cargo or Git workspace. Sanctifier should attach automatically.

## Using it alongside rust-analyzer

Keep `rust-analyzer` enabled. Neovim supports multiple LSP clients on the same Rust buffer, so rust-analyzer can continue to provide completion, navigation, refactoring, and its own diagnostics while Sanctifier contributes security diagnostics and hover details.

Sanctifier's current server advertises full-document text synchronization, hover, and diagnostics. It does not currently advertise completion, code actions, formatting, or workspace diagnostics.

## Verify and troubleshoot

Inside Neovim:

```vim
:checkhealth vim.lsp
:LspInfo
```

Confirm that an active client named `sanctifier` is attached to the Rust buffer.

If it does not attach:

1. Run `:set filetype?` and confirm the buffer is `rust`.
2. Run `:echo executable('sanctifier')`; it should return `1`.
3. Check that an ancestor directory contains `Cargo.toml` or `.git`.
4. Run `:checkhealth vim.lsp` and inspect the Sanctifier command and startup error.
5. Run `sanctifier lsp --help` in the same shell environment Neovim inherits to confirm the installed CLI exposes the LSP command.

The integration does not require a second `sanctifier-lsp` executable: the CLI starts the LSP server in-process.
