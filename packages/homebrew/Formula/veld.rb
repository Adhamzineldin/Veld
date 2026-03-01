# Veld Homebrew Formula
#
# Install: brew install veld-dev/tap/veld
# Or add the tap: brew tap veld-dev/tap && brew install veld
#
# To update this formula after a release:
#   1. Update the version, url, and sha256 values
#   2. Submit to the homebrew-tap repository

class Veld < Formula
  desc "Contract-first, multi-stack API code generator"
  homepage "https://github.com/Adhamzineldin/Veld"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Adhamzineldin/Veld/releases/download/v#{version}/veld-darwin-arm64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_DARWIN_ARM64"
    else
      url "https://github.com/Adhamzineldin/Veld/releases/download/v#{version}/veld-darwin-amd64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_DARWIN_AMD64"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/Adhamzineldin/Veld/releases/download/v#{version}/veld-linux-arm64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_ARM64"
    else
      url "https://github.com/Adhamzineldin/Veld/releases/download/v#{version}/veld-linux-amd64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_AMD64"
    end
  end

  def install
    bin.install "veld"
  end

  test do
    assert_match "#{version}", shell_output("#{bin}/veld --version")
  end
end

