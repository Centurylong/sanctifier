# Installing Sanctifier

Sanctifier ships pre-built binaries for macOS (Intel & Apple Silicon), Linux
(x86-64 & aarch64), and Windows (x86-64).  Pick the method that best fits your
workflow.

## Homebrew (macOS & Linux)

```bash
brew tap Centurylong/sanctifier
brew install sanctifier
```

This downloads a pre-built binary that is statically linked against z3, so no
extra system libraries are required.

To upgrade later:

```bash
brew upgrade sanctifier
```

## cargo-binstall

[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) downloads the
correct pre-built binary for your platform straight from GitHub Releases,
falling back to a source build if no binary is available.

```bash
# Install cargo-binstall itself (once)
cargo install cargo-binstall

# Install sanctifier
cargo binstall sanctifier-cli
```

## Manual download

Pre-built archives are attached to every [GitHub
Release](https://github.com/Centurylong/sanctifier/releases).  Download the
archive for your platform, verify the checksum, and place the binary on your
`PATH`.

| Platform        | Archive name                                               |
|-----------------|------------------------------------------------------------|
| macOS (Apple Silicon) | `sanctifier-<version>-aarch64-apple-darwin.tar.gz`  |
| macOS (Intel)   | `sanctifier-<version>-x86_64-apple-darwin.tar.gz`         |
| Linux x86-64    | `sanctifier-<version>-x86_64-unknown-linux-gnu.tar.gz`    |
| Linux aarch64   | `sanctifier-<version>-aarch64-unknown-linux-gnu.tar.gz`   |
| Windows x86-64  | `sanctifier-<version>-x86_64-pc-windows-msvc.zip`         |

Verify the download against `SHA256SUMS` (also attached to each release):

```bash
sha256sum --check SHA256SUMS
```

## Build from source with cargo

Requires Rust 1.78+ and `libz3` (or a compiler toolchain to build z3 from source).

```bash
# From crates.io (once published)
cargo install sanctifier-cli

# From the git repository
cargo install --git https://github.com/Centurylong/sanctifier sanctifier-cli
```

On Debian/Ubuntu you can satisfy the z3 build dependency with:

```bash
sudo apt-get install libz3-dev cmake
```

On macOS:

```bash
brew install z3 cmake
```

## Verifying a release (optional)

Each release attaches a `SHA256SUMS` file.  After downloading a binary archive:

```bash
# Example for Linux x86-64
curl -LO https://github.com/Centurylong/sanctifier/releases/download/v0.1.0/sanctifier-0.1.0-x86_64-unknown-linux-gnu.tar.gz
curl -LO https://github.com/Centurylong/sanctifier/releases/download/v0.1.0/SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS
```
