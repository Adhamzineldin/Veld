# Veld — Package Manager Wrappers

This directory contains wrapper packages for distributing the Veld CLI through
various package managers. Each wrapper downloads the pre-built Go binary for the
user's platform from GitHub Releases.

## Supported Package Managers

| Manager    | Package                  | Install Command                                       |
|------------|--------------------------|-------------------------------------------------------|
| **npm**    | `veld`                   | `npm install veld` / `npx veld generate`              |
| **pip**    | `veld`                   | `pip install veld`                                    |
| **Homebrew** | `veld-dev/tap/veld`    | `brew install veld-dev/tap/veld`                      |
| **Go**     | `github.com/veld-dev/veld` | `go install github.com/veld-dev/veld/cmd/veld@latest` |
| **Composer** | `veld-dev/veld`        | `composer require veld-dev/veld`                      |

## How they work

All wrappers follow the same pattern:

1. On install (or first run), detect platform and architecture
2. Download the pre-built binary from `https://github.com/veld-dev/veld/releases/download/v{VERSION}/veld-{os}-{arch}`
3. Cache the binary locally
4. Proxy all CLI arguments to the binary

If the download fails, each wrapper falls back to `go install`.

## Release workflow

Releases are **fully automated** via GitHub Actions (`.github/workflows/release.yml`).

### How to release

```bash
# 1. Commit your changes
git add -A && git commit -m "feat: your changes"

# 2. Tag with a version
git tag v0.2.0

# 3. Push the tag
git push origin v0.2.0
```

That's it. The CI pipeline will:

1. **Run tests** — `go test ./... -race`
2. **Build binaries** — cross-compile for linux/darwin/windows × amd64/arm64
3. **Create GitHub Release** — upload all archives + SHA256 checksums + auto-generated changelog
4. **Publish to npm** — updates version and runs `npm publish`
5. **Publish to PyPI** — updates version and runs `twine upload`
6. **Update Homebrew formula** — computes SHA256 hashes and patches `Formula/veld.rb`

### Required secrets

Set these in your GitHub repo → Settings → Secrets → Actions:

| Secret | Required for | How to get |
|--------|-------------|------------|
| `NPM_TOKEN` | npm publish | npmjs.com → Access Tokens → Generate |
| `PYPI_TOKEN` | pip publish | pypi.org → Account Settings → API tokens |
| `HOMEBREW_TAP_TOKEN` | Homebrew tap push (optional) | GitHub PAT with `repo` scope on the tap repo |

> `GITHUB_TOKEN` is provided automatically — no setup needed for the release itself.

### Pre-release versions

Tags with a hyphen (e.g. `v0.2.0-beta.1`) are published as **pre-release** on GitHub
and **skip** npm/pip/Homebrew publishing.

## Directory structure

```
packages/
├── npm/              # npm wrapper (Node.js)
│   ├── package.json
│   ├── bin/veld.js
│   ├── install.js
│   └── README.md
├── pip/              # pip wrapper (Python)
│   ├── pyproject.toml
│   ├── veld/__init__.py
│   ├── veld/__main__.py
│   └── README.md
├── homebrew/         # Homebrew formula (macOS/Linux)
│   ├── Formula/veld.rb
│   └── README.md
├── composer/         # Composer wrapper (PHP)
│   ├── composer.json
│   ├── bin/veld
│   ├── src/Installer.php
│   └── README.md
└── go/               # Go install instructions
    └── README.md
```

