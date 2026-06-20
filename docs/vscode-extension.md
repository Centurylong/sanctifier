# Sanctifier VS Code Extension

This document describes the new VS Code extension package under `editors/vscode`.

## What it does

- Analyzes Rust/Soroban source files on save using `@sanctifier/sdk`.
- Surfaces findings as inline diagnostics in the editor.
- Attaches quick links to finding code documentation for each diagnostic.

## Installation

```bash
cd editors/vscode
npm install
npm run compile
```

## Packaging

```bash
cd editors/vscode
npm run package
```

## Publishing

```bash
cd editors/vscode
npm run publish
```

## Configuration

The extension exposes the following settings:

- `sanctifier-vscode.enabled`
- `sanctifier-vscode.analyzeOnSave`
- `sanctifier-vscode.failOn`
- `sanctifier-vscode.enabledRules`

These settings are defined in the extension manifest at `editors/vscode/package.json`.
