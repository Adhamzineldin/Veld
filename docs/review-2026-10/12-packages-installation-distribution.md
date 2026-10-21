# 12. Package management, installation and generated-SDK distribution

Three different concerns are kept separate here. Evidence: CLI/distribution audit track (F-CLI-13 to F-CLI-17), ADR-008.

## 12.1 Installing Veld itself

### 12.1.1 State at 6a47f81 (verified)

| Channel | Directory | How it obtains the binary | Checksum | Platform selection | Grade |
|---|---|---|---|---|---|
| npm | `packages/npm` | `install.js` postinstall downloads from GitHub Releases `latest`, not the wrapper's own version; `npm install` exits 0 even when the download fails, then `veld` fails (reproduced) | none, although the release publishes `checksums.txt` | no windows-arm64 asset; no proxy support | needs work |
| pip | `packages/pip` | bundles all five binaries in the wheel; `veld/__main__.py` uses `str \| None` at import time, which fails on Python 3.8/3.9 while `requires-python >= 3.8` (source-read) | none | n/a (bundled) | needs work; package name `maayn-veld` disagrees with README `veld` |
| Homebrew | `packages/homebrew` | formula with `PLACEHOLDER` sha256 values; release job edits the formula on the runner and never pushes it | placeholder | n/a | placeholder |
| Go | `packages/go`, `go install` | works from the README; binary reports version 0.1.0 (F-CLI-13) | Go module checksum DB | all | ready (version bug) |
| NuGet / Chocolatey | `packages/nuget`, `packages/chocolatey` | hardcoded 0.1.0; placeholder sha256; nuspec links a LICENSE file that does not exist | none | no windows-arm64 | needs work |
| apt (nfpm) | `packages/apt` | installs to `/usr/local/bin` | n/a | linux | needs work |
| Composer | `packages/composer` | no release job; version constant never updated; dependency post-install hooks do not run for consumers; cannot be listed on Packagist from a monorepo subdirectory | none | n/a | placeholder |
| GitHub Releases | `release.yml` | five targets, `checksums.txt`; no signing, SBOM or provenance; actions pinned by tag | yes (file published) | linux/darwin amd64+arm64, windows amd64 | partial |

### 12.1.2 Decision: channels for graduation

| Priority | Channel | Reason | Work |
|---|---|---|---|
| 1 | GitHub Releases via GoReleaser: archives, `checksums.txt`, cosign signatures, SBOM, GitHub provenance attestation; add `windows-arm64` | source of truth for every wrapper | 8 to 12 h (T5) |
| 2 | npm wrapper | largest audience for the vertical slice; `npx veld` onboarding | pin to the wrapper's version, verify sha256 from `checksums.txt`, fail loudly, honour `HTTPS_PROXY`, skip download when `VELD_BINARY` is set; 4 to 6 h (T5) |
| 3 | Homebrew tap | macOS developers | GoReleaser `brews` publishes the formula to a tap repository; 2 to 3 h |
| 4 | `go install` | zero-maintenance | version from `debug.ReadBuildInfo`; 1 h |
| 5 | pip | Python backend users | fix 3.8/3.9 syntax or raise `requires-python` to 3.10; download-on-first-run instead of bundling five binaries; 4 to 6 h |
| defer | NuGet, Chocolatey, apt, Composer | small audiences, each costs a release job to keep honest | mark as community-maintained, remove placeholders from the repo or move to `packages/experimental/` with a README stating they are unsupported |

Wrapper versus native reimplementation: wrappers only. Every wrapper downloads the Go binary; none reimplements Veld. Uninstall is the package manager's own removal plus `~/.veld` (credentials, cache), which `veld logout --all` and `veld cache clean` must document.

Offline and interrupted installation: wrappers write to a temporary file and rename on success; a partial download never leaves a half-written `veld` on PATH; a checksum mismatch deletes the file and prints the expected and actual hash. Lifecycle-script restriction: when `npm` runs with `--ignore-scripts`, the npm wrapper's `bin` shim prints a one-line instruction to run `npx veld-install` instead of failing silently.

## 12.2 Installing Veld ecosystem artifacts (contracts and plugins)

Decision record: ADR-008. Summary of the smallest useful system:

| Concern | Decision |
|---|---|
| Package types | Contract packages (data only: `.veld`, manifest, README, LICENSE) are hosted by the Veld registry. Emitter plugins are executables and are distributed through npm, Go modules or GitHub Releases; the registry does not host executables at graduation. Different trust policies: contracts are parsed, never executed; plugins run as the user. |
| Manifest | `veld.package.json` (`manifestVersion`, `name` as `@org/name`, semver `version`, `entry`, `dependencies`, `license`, `description`). Validated on `veld push`. |
| Namespaces | `@org` scopes map one-to-one to registry organizations; each scope is bound to exactly one registry URL in `veld.config.json` (`registries`). No fallback across registries, which closes the dependency-confusion path. |
| Version constraints | exact, caret, `1.x`; semver only |
| Resolution | highest version satisfying all constraints; one version per package per project; conflicts are errors naming both requesters; transitive dependencies resolved client-side from the registry's version metadata |
| Lockfile | `veld.lock.json`: name, version, resolved URL, `sha256-` integrity, dependency tree; `veld install --frozen` for CI |
| Integrity | checksum always required before extraction (today optional, `cmd/veld/registry.go:232`); lockfile hash wins over server hash |
| Cache | `~/.veld/cache/<registry-host>/<org>/<name>/<version>.tgz` and a content-addressed `sha256/` directory; `--offline` serves from cache only |
| Local and workspace packages | `"dependencies": { "@acme/common": "file:../common" }` resolved without the registry; workspace entries can `consumes` each other as today |
| Credentials | `~/.veld/credentials.json` keyed by registry URL (as today); token sent only to its own URL; `DefaultRegistry()` must stop picking a random stored registry (`internal/registry/credentials.go:105-107` iterates a map) |

What is not built: a general-purpose package manager, mirrors, proxies, signing of packages (checksums only), and yanking semantics beyond `deprecated` (chapter 13).

Cost: manifest and pack filtering 6 to 10 h; lockfile and resolver 16 to 24 h; cache and offline 8 to 12 h; scoped registries and tests 6 to 10 h. Owner T3, reviewer T1. Scheduled in M4.

## 12.3 Publishing generated SDKs into target ecosystems

Veld generates source, not packages. The recommended path is to generate into a directory that the user's own package tooling owns:

- TypeScript: generate into `packages/api-client/src/` of the user's monorepo; the user's `package.json` publishes it. Veld emits a `package.json` only when asked (`--emit-package-json`), with `name`, `version` from the contract's declared version, `types` and `exports`, no dependencies.
- Python: generate into a package directory with `__init__.py`; the user adds `pyproject.toml`. Veld emits a `py.typed` marker.
- Go: generate a package with its own `go.mod` only in `--standalone` mode; by default generate inside the user's module.

Veld does not publish to npm, PyPI or pkg.go.dev on behalf of the user at graduation. A `veld publish-sdk` command is a post-graduation idea only.

## 12.4 Success criteria

- A clean Windows, macOS and Linux machine installs Veld through npm and through the release archive, with checksum verification demonstrated by corrupting one byte and observing the refusal.
- `veld pull` of a package from a self-hosted registry produces a lockfile; a second `veld install --frozen` on another machine yields byte-identical `veld/packages/`.
- `veld push` of a package containing a `.veld-cache.json`, a dotfile and a nested `node_modules` directory packs only the allowed files (test with the tarball listing).
