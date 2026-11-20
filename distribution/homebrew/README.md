# Veld - Homebrew

```bash
brew install maayn-veld/tap/maayn-veld
```

The formula downloads the release archive for your platform and Homebrew verifies its sha256.
After publishing a release, run `./update-formula.sh SHA256SUMS` (from the release assets) to fill
in the checksums, then copy `Formula/maayn-veld.rb` to the tap repository.

Version: 2.0.0-beta.1. Assets: `veld-<target-triple>.tar.gz`, see `.github/workflows/release.yml`.
