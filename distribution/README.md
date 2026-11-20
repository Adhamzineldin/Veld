# Veld distribution wrappers (2.x)

The `veld` CLI is one self-contained Rust binary. The wrappers here only download the release
binary for the user's platform, verify its SHA256 against the release's `SHA256SUMS`, and run it.

| Channel | Package | Install |
|---|---|---|
| npm | `@maayn/veld` | `npm install -g @maayn/veld` |
| pip | `maayn-veld` | `pip install maayn-veld` |
| Homebrew | `maayn-veld/tap/maayn-veld` | `brew install maayn-veld/tap/maayn-veld` |
| source | | `cargo install --git https://github.com/Adhamzineldin/Veld veld` |

All three wrappers are at version `2.0.0-beta.1` and fetch from
`https://github.com/Adhamzineldin/Veld/releases/download/v<version>/`.

## Release assets

Built by `.github/workflows/release.yml` when a `v*` tag is pushed:

| Asset | Platform |
|---|---|
| `veld-x86_64-unknown-linux-musl.tar.gz` | Linux x64 |
| `veld-aarch64-unknown-linux-musl.tar.gz` | Linux arm64 |
| `veld-x86_64-apple-darwin.tar.gz` | macOS x64 |
| `veld-aarch64-apple-darwin.tar.gz` | macOS arm64 |
| `veld-x86_64-pc-windows-msvc.zip` | Windows x64 |
| `veld-registry-linux-amd64.tar.gz`, `veld-registry-linux-arm64.tar.gz` | the Go registry server |
| `SHA256SUMS` | checksums of all of the above |

Each `veld` archive contains the `veld` (or `veld.exe`) binary at its root.

## Maintained and archived channels

npm, pip and Homebrew are maintained in 2.x. The Go (`go install`), NuGet, Chocolatey, apt and
Composer channels wrapped the v1 Go binary and are **not maintained for 2.0**. They are kept for
reference in [`archive/v1/distribution/`](../archive/v1/distribution). They can come back
later as wrappers around the same release assets.

## Releasing

1. Tag the commit: `git tag v2.0.0-beta.1 && git push origin v2.0.0-beta.1`.
2. The workflow builds, checksums and uploads the assets (tags with a `-` are marked pre-release).
3. Publish the wrappers (see [PUBLISHING.md](PUBLISHING.md)). For Homebrew run
   `homebrew/update-formula.sh SHA256SUMS` first.

Optional workflow secrets: `DOCKERHUB_USERNAME` and `DOCKERHUB_TOKEN` enable the registry Docker image push.
