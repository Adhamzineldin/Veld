# Veld distribution wrappers (2.x)

The `veld` CLI is one self-contained Rust binary. The wrappers here only download the release
binary for the user's platform, verify its SHA256 against the release's `SHA256SUMS`, and run it.

| Channel | Package | Install |
|---|---|---|
| npm | `@maayn/veld` | `npm install -g @maayn/veld` |
| pip | `maayn-veld` | `pip install maayn-veld` |
| Homebrew | `maayn-veld/tap/maayn-veld` | `brew install maayn-veld/tap/maayn-veld` |
| apt / rpm | `veld` (.deb, .rpm on the GitHub release) | `sudo apt install ./veld_<version>_amd64.deb` or `sudo rpm -i veld-<version>-1.x86_64.rpm` |
| Chocolatey | `veld` | `choco install veld --pre` |
| Scoop | `veld.json` (release asset, or a bucket) | `scoop install https://github.com/Adhamzineldin/Veld/releases/latest/download/veld.json` |
| source | | `cargo install --git https://github.com/Adhamzineldin/Veld veld` |

The npm, pip and Homebrew wrappers are at version `2.0.0-beta.1` and fetch from
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

## Maintained channels and what was left out

npm, pip, Homebrew, apt/rpm (nfpm), Chocolatey and Scoop are maintained in 2.x. Every one of them only
fetches or repackages the release assets above and verifies their SHA256 (the Chocolatey script and the
Scoop manifest check the hash themselves; the deb and rpm embed the verified binary).

v1 had five more channels in [`archive/v1/distribution/`](../archive/v1/distribution). Their status:

| v1 channel | 2.x | Why |
|---|---|---|
| apt (`nfpm.yaml`) | restored: `apt/nfpm.yaml`, also builds rpm | the v1 file installed the Go binary to `/usr/local/bin`; 2.x installs the static binary to `/usr/bin`, adds the licence, and is arch-aware |
| Chocolatey | restored: `chocolatey/` | same layout, new asset name and checksum filled from `SHA256SUMS` |
| Composer | not restored | it wrapped the v1 binary for PHP projects; the PHP target is gone in 2.0 (owner decision), and the npm and pip wrappers already cover polyglot repositories |
| NuGet (dotnet tool) | not restored | same reason for the C# target; a `dotnet tool` wrapper adds a runtime dependency on .NET for no gain |
| `go install` | impossible | the compiler is Rust now; `cargo install --git` replaces it and the registry server is a separate Go module (`registry/`) |

The committed `chocolatey/tools/chocolateyinstall.ps1` and `scoop/veld.json` hold a
`PLACEHOLDER_SHA256_...` hash: they are templates. `package.sh choco|scoop <version> SHA256SUMS <out>` fills them,
and `tests/packages.test.sh` checks that.

## Releasing

1. Tag the commit: `git tag v2.0.0-beta.1 && git push origin v2.0.0-beta.1`.
2. The workflow builds, checksums and uploads the assets (tags with a `-` are marked pre-release).
3. Publish the wrappers (see [PUBLISHING.md](PUBLISHING.md)); the .deb, .rpm, .nupkg and `veld.json` are already attached to the release. For Homebrew run
   `homebrew/update-formula.sh SHA256SUMS` first.

Optional workflow secrets: `DOCKERHUB_USERNAME` and `DOCKERHUB_TOKEN` enable the registry Docker image push.
