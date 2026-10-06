# Emacs integration (Eglot and lsp-mode)

Sanctifier ships an LSP server through the CLI:

```text
sanctifier lsp --stdio
```

The server uses standard LSP over stdin/stdout, publishes diagnostics when Rust
documents are opened, changed, or saved, and provides hover text for findings.
No editor-specific server binary is required.

## Prerequisite

Install the Sanctifier CLI and make sure `sanctifier` is on the same `PATH`
Emacs inherits. See [Installation](installation.md) for supported install
methods.

The `--stdio` transport is required. Starting the command manually is not a
useful health check because an LSP server waits for protocol frames on stdin.

## Eglot

Built-in Eglot can launch Sanctifier directly:

```elisp
(with-eval-after-load 'eglot
  (add-to-list
   'eglot-server-programs
   '((rust-mode rust-ts-mode) . ("sanctifier" "lsp" "--stdio"))))

(add-hook 'rust-mode-hook #'eglot-ensure)
(add-hook 'rust-ts-mode-hook #'eglot-ensure)
```

This makes Sanctifier the Eglot server selected for Rust buffers by that
configuration. If you already use Eglot with `rust-analyzer`, keep that setup
instead of replacing it blindly; Eglot selects a server program for the mode
rather than treating this recipe as an automatic second Rust server.

To inspect connection problems, use `M-x eglot-events-buffer`. Diagnostics
reported by Sanctifier use `sanctifier` as their LSP source.

## lsp-mode

`lsp-mode` can register Sanctifier as an add-on Rust client so it can run
alongside a primary Rust language server:

```elisp
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection
    (lsp-stdio-connection '("sanctifier" "lsp" "--stdio"))
    :activation-fn (lsp-activate-on "rust")
    :server-id 'sanctifier
    :add-on? t
    :priority -1)))

(add-hook 'rust-mode-hook #'lsp-deferred)
(add-hook 'rust-ts-mode-hook #'lsp-deferred)
```

If `sanctifier` is not on Emacs' `PATH`, replace the first command element
with an absolute executable path. Use `M-x lsp-workspace-show-log` when the
client does not start.

## What to expect

The current Sanctifier server advertises full-document text synchronization,
hover support, and document diagnostics. It analyzes the text Emacs sends over
LSP, so unsaved edits can update findings without rereading the file from disk.

The server does not currently advertise code actions or workspace diagnostics;
this recipe therefore does not promise editor quick-fixes or whole-workspace
diagnostic requests.

## Remove the integration

For Eglot, remove the Sanctifier entry from `eglot-server-programs` and the
hooks above. For lsp-mode, remove the `sanctifier` client registration from
your configuration and restart Emacs.
