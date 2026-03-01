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

To publish a new version:

1. **Build binaries** for all platforms (CI recommended):
   - `veld-linux-amd64.tar.gz`
   - `veld-linux-arm64.tar.gz`
   - `veld-darwin-amd64.tar.gz`
   - `veld-darwin-arm64.tar.gz`
   - `veld-windows-amd64.zip`

2. **Create GitHub Release** tagged `v{VERSION}` with all archives attached

3. **Publish packages**:
   ```bash
   # npm
   cd packages/npm && npm publish

   # pip
   cd packages/pip && python -m build && twine upload dist/*

   # Homebrew — update sha256 in Formula/veld.rb, push to homebrew-tap repo

   # Composer
   # Tag the repo — Packagist auto-updates from GitHub

   # Go — automatic via go install @latest
   ```

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

