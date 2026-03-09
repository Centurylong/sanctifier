# Contributing to Sanctifier 🛡️

Thank you for your interest in contributing to Sanctifier! This document provides guidelines for building, testing, and formatting your code to ensure a smooth contribution process.

## 🛠 Prerequisites

To build and test Sanctifier, you will need the following tools installed:

- **Rust**: The latest stable version. Install via [rustup](https://rustup.rs/).
- **Node.js**: Version 20 or higher (for the frontend).
- **Stellar CLI**: Recommended for local Soroban development.
- **Kani**: Required for running formal verification tests.

## 📁 Project Structure

Sanctifier is a hybrid project:
- `tooling/`: Rust-based core engine, CLI, and SDK.
- `contracts/`: Soroban contract templates and security guards.
- `frontend/`: Next.js dashboard for security visualization.
- `docs/`: Technical documentation and security guides.

## 🏗 Building Locally

### Rust Tooling & Contracts
From the root of the repository, you can build the entire Rust workspace:

```bash
cargo build --workspace
```

To build only the CLI:
```bash
cargo build -p sanctifier-cli
```

### Frontend
Navigate to the `frontend/` directory and install dependencies:

```bash
cd frontend
npm install
npm run dev
```

## 🧪 Running Tests

### Rust Tests
Run all tests in the workspace:

```bash
cargo test --workspace
```

### Frontend Tests
Currently, the frontend does not have automated tests. Ensure the project builds without errors:

```bash
cd frontend
npm run build
```

## 🎨 Formatting & Linting

We maintain high code quality standards. Please ensure your code passes the following checks before opening a Pull Request.

### Rust
Use `rustfmt` and `clippy`:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Frontend
Run ESLint:

```bash
cd frontend
npm run lint
```

## 🚀 Pull Request Process

1. **Create an Issue**: Before starting work, please ensure there is an issue tracking the feature or bug.
2. **Branching**: Create a branch with a descriptive name (e.g., `fix/issue-123-description` or `feat/new-security-rule`).
3. **Upstream Sync**: Always fetch and merge the latest changes from the upstream `main` branch before submitting.
4. **Open PR**: Provide a clear description of your changes and link to the issue using `Closes #<issue-number>`.

---
Thank you for helping us build a more secure Stellar ecosystem! 🛡️
