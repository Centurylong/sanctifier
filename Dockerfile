# Stage 1: Build the Sanctifier CLI
#
# tooling/sanctifier-cli is its own Cargo workspace (see its `[workspace]
# members = ["."]` and .github/workflows/ci.yml), separate from the root
# workspace, so it is built from its own directory with its own Cargo.lock —
# `cargo build --package sanctifier-cli` from the repo root cannot find it.
FROM rust:1.85-slim AS builder

# cmake + a C++ toolchain build z3 from source (the CLI depends on the z3
# crate directly, and on sanctifier-core's "smt" feature, which also pulls it
# in) when the `static-link-z3` feature below is enabled, so the runtime
# image needs no libz3 package installed at all.
# z3-sys 0.8.1 vendors an old z3 CMakeLists.txt whose
# `cmake_minimum_required` predates the 3.5 floor recent CMake enforces;
# this is the documented escape hatch (cmake >= 3.31).
ENV CMAKE_POLICY_VERSION_MINIMUM=3.5
RUN apt-get update \
    && apt-get install -y pkg-config libssl-dev cmake build-essential libdbus-1-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for better layer caching. sanctifier-cli depends on
# sanctifier-core via a relative path, so both manifests are needed to
# resolve the dependency graph before the real source is copied in.
COPY tooling/sanctifier-core/Cargo.toml tooling/sanctifier-core/Cargo.toml
COPY tooling/sanctifier-cli/Cargo.toml tooling/sanctifier-cli/Cargo.lock tooling/sanctifier-cli/

RUN mkdir -p tooling/sanctifier-core/src && echo "pub fn dummy() {}" > tooling/sanctifier-core/src/lib.rs \
    && mkdir -p tooling/sanctifier-cli/src && echo "fn main() {}" > tooling/sanctifier-cli/src/main.rs \
    && (cd tooling/sanctifier-cli && cargo build --release --features static-link-z3) 2>/dev/null || true

# Copy the actual source and build the real binary.
COPY tooling/sanctifier-core tooling/sanctifier-core
COPY tooling/sanctifier-cli tooling/sanctifier-cli
RUN cd tooling/sanctifier-cli && cargo build --release --features static-link-z3

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# The [[bin]] in tooling/sanctifier-cli/Cargo.toml names the binary
# "sanctifier", not "sanctifier-cli".
COPY --from=builder /app/tooling/sanctifier-cli/target/release/sanctifier /usr/local/bin/sanctifier

WORKDIR /workspace

ENTRYPOINT ["sanctifier"]
CMD ["--help"]
