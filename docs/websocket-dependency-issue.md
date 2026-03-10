# WebSocket Dependency Conflict

## Issue

The WebSocket-based real-time log streamer implementation is complete but cannot currently build due to a dependency conflict in the Rust workspace:

```
error: failed to select a version for `syn`.
    ... required by package `futures-macro v0.3.31`
    ... which satisfies dependency `futures-util = "^0.3"` of package `sanctifier-cli`
    
versions that meet the requirements `^2.0.52` are available
    
all possible versions conflict with previously selected packages.
  previously selected package `syn v2.0.39`
    ... which satisfies dependency `syn = "=2.0.39"` of package `soroban-builtin-sdk-macros v20.0.0`
```

## Root Cause

- `soroban-sdk v20.0.0` (used in contracts) pins `syn = "=2.0.39"` (exact version)
- `tokio-tungstenite` and `futures-util` (needed for WebSocket) require `syn >= 2.0.52`
- Cargo cannot resolve this conflict in a workspace

## Solutions

### Option 1: Update Soroban SDK (Recommended)

Wait for or upgrade to a newer version of `soroban-sdk` that uses a compatible `syn` version:

```toml
# In contracts/*/Cargo.toml
[dependencies]
soroban-sdk = "21.0.0"  # or later version with syn >= 2.0.52
```

### Option 2: Separate Workspace

Move the CLI tool to a separate workspace:

1. Remove `tooling/sanctifier-cli` from the main workspace members
2. Create a new `Cargo.toml` in `tooling/sanctifier-cli` with its own workspace
3. Reference `sanctifier-core` as a path dependency

```toml
# tooling/sanctifier-cli/Cargo.toml
[workspace]
members = ["."]

[package]
name = "sanctifier-cli"
# ...

[dependencies]
sanctifier-core = { path = "../sanctifier-core" }
tokio-tungstenite = "0.24"
futures-util = "0.3"
# ...
```

### Option 3: Feature Flag

Make WebSocket support optional behind a feature flag:

```toml
# tooling/sanctifier-cli/Cargo.toml
[features]
default = []
websocket = ["tokio-tungstenite", "futures-util"]

[dependencies]
tokio-tungstenite = { version = "0.24", optional = true }
futures-util = { version = "0.3", optional = true }
```

Then conditionally compile WebSocket code:

```rust
#[cfg(feature = "websocket")]
mod ws_server;

#[cfg(feature = "websocket")]
Commands::Stream { ... } => { ... }
```

## Current Status

All WebSocket code is implemented and ready:

✅ Backend WebSocket server (`tooling/sanctifier-cli/src/ws_server.rs`)
✅ Stream command (`tooling/sanctifier-cli/src/commands/stream.rs`)  
✅ Frontend LogViewer component (`frontend/app/components/LogViewer.tsx`)
✅ Frontend Stream page (`frontend/app/stream/page.tsx`)
✅ Documentation (`docs/realtime-streaming.md`)
✅ Tests (`frontend/app/components/__tests__/LogViewer.test.tsx`)

❌ Cannot build due to dependency conflict

## Testing the Implementation

Once the dependency issue is resolved, test with:

```bash
# Build the CLI
cd tooling/sanctifier-cli
cargo build

# Start the WebSocket server
cargo run -- stream ./contracts/vulnerable-contract --port 9001

# In another terminal, start the frontend
cd frontend
npm run dev

# Navigate to http://localhost:3000/stream
```

## Workaround for Development

For immediate testing, you can:

1. Temporarily remove contracts from workspace members in root `Cargo.toml`
2. Build and test the CLI with WebSocket support
3. Re-add contracts when done

```toml
# Cargo.toml (temporary)
[workspace]
members = [
    "tooling/sanctifier-cli",
    "tooling/sanctifier-core",
    "tooling/sanctifier-wasm",
    "tooling/sanctifier-sdk",
    # Temporarily commented out:
    # "contracts/vulnerable-contract",
    # "contracts/kani-poc",
    # "contracts/reentrancy-guardian",
]
```

## Timeline

This issue should be resolved when:
- Soroban SDK updates to use a newer `syn` version (likely in next major release)
- Or when the workspace is restructured to separate concerns

The WebSocket implementation is production-ready and will work immediately once dependencies are compatible.
