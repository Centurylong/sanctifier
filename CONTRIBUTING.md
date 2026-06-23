# Contributing to Sanctifier

Thank you for your interest in contributing to Sanctifier! This guide will help you get started with setting up your development environment and contributing new security detectors.

## Codebase Architecture

Sanctifier consists of the following components:
- `tooling/sanctifier-core`: The core static analysis engine.
- `tooling/sanctifier-cli`: The CLI interface wrapper around the core library.
- `contracts/`: Reference Soroban smart contracts and verification targets.

## Writing Custom Detectors

We have a dedicated guide outlining the pipeline, implementation patterns, and testing process for adding custom static analysis rules:

👉 **[Detector Cookbook](docs/detector-cookbook.md)**

Please review the cookbook before writing any detectors. It provides three complete examples ranging from syntactic rules to data-flow tracking.

## Testing Guidelines

Every detector must be covered by a golden `insta` snapshot. For instructions on how to run, update, and review these snapshots, see [tooling/sanctifier-core/tests/README.md](tooling/sanctifier-core/tests/README.md).

```bash
# Run all core tests and snapshot assertions
cargo test -p sanctifier-core --all-features
```
