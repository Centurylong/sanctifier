# This formula is kept in the main repo so that `brew tap Centurylong/sanctifier`
# picks it up automatically.  The SHA-256 values below are updated by the
# release workflow on every tagged release.
#
# Manual tap install:
#   brew tap Centurylong/sanctifier
#   brew install sanctifier
class Sanctifier < Formula
  desc "Security and formal verification CLI for Stellar Soroban smart contracts"
  homepage "https://github.com/Centurylong/sanctifier"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Centurylong/sanctifier/releases/download/v#{version}/sanctifier-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_AARCH64_APPLE_DARWIN"
    end
    on_intel do
      url "https://github.com/Centurylong/sanctifier/releases/download/v#{version}/sanctifier-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_X86_64_APPLE_DARWIN"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Centurylong/sanctifier/releases/download/v#{version}/sanctifier-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_X86_64_LINUX"
    end
  end

  def install
    bin.install "sanctifier"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/sanctifier --version")
  end
end
