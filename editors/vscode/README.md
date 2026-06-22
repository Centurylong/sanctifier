# Sanctifier VS Code

VS Code extension that runs Sanctifier analysis on save and surfaces findings as inline diagnostics with quick links to the finding-code docs.

## Features

- Runs Sanctifier analysis when Rust files are saved.
- Maps findings to VS Code diagnostics with severity and range.
- `code` values are linked to the Sanctifier finding docs.
- Supports configuration for `failOn`, rule filtering, and enable/disable.

## Requirements

- VS Code 1.80 or newer.
- Node 18+ for packaging and extension development.
- The local `@sanctifier/sdk` package from this repo.

## Getting started

```bash
cd editors/vscode
npm install
npm run compile
code .
```

## Extension settings

- `sanctifier-vscode.enabled`: Enable or disable analysis.
- `sanctifier-vscode.analyzeOnSave`: Run analysis on file save.
- `sanctifier-vscode.failOn`: Treat findings at or above this severity as failure.
- `sanctifier-vscode.enabledRules`: Restrict analysis to specific finding codes or rule names.

## Packaging

Build the extension and package it into a `.vsix` file:

```bash
cd editors/vscode
npm install
npm run package
```

## Publishing

Publish the extension with `vsce`:

```bash
cd editors/vscode
npm install
npm run publish
```

If you do not already have a publisher configured, follow the VS Code Marketplace documentation to create one and set the `publisher` field in `package.json`.
