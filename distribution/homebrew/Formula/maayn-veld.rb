# Veld Homebrew formula (2.x)
#
# Install: brew install maayn-veld/tap/maayn-veld
#
# The assets are the ones built by .github/workflows/release.yml. After a release, fill in the
# sha256 values from the release's SHA256SUMS file with: ./update-formula.sh <SHA256SUMS>
# Homebrew verifies every download against these checksums.

class MaaynVeld < Formula
  desc "Contract-first API code generator"
  homepage "https://github.com/Adhamzineldin/Veld"
  version "2.0.0-beta.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Adhamzineldin/Veld/releases/download/v#{version}/veld-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_AARCH64_APPLE_DARWIN"
    else
      url "https://github.com/Adhamzineldin/Veld/releases/download/v#{version}/veld-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64_APPLE_DARWIN"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/Adhamzineldin/Veld/releases/download/v#{version}/veld-aarch64-unknown-linux-musl.tar.gz"
      sha256 "PLACEHOLDER_SHA256_AARCH64_UNKNOWN_LINUX_MUSL"
    else
      url "https://github.com/Adhamzineldin/Veld/releases/download/v#{version}/veld-x86_64-unknown-linux-musl.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64_UNKNOWN_LINUX_MUSL"
    end
  end

  def install
    bin.install "veld"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/veld --version")
  end
end
