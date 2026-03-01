# Veld — npm package

> Contract-first, multi-stack API code generator.

## Install

```bash
npm install @maayn/veld
# or
npx @maayn/veld generate
```

## Usage

After installing, the `veld` CLI is available:

```bash
veld init                    # Scaffold a new project
veld generate                # Generate from veld.config.json
veld generate --dry-run      # Preview what would be generated
veld watch                   # Auto-regenerate on file changes
veld validate                # Check contracts for errors
veld clean                   # Remove generated output
veld openapi                 # Export OpenAPI 3.0 spec
```

## How it works

This npm package is a thin wrapper around the Veld Go binary. On `npm install`,
a postinstall script downloads the correct pre-built binary for your platform
from [GitHub Releases](https://github.com/Adhamzineldin/Veld/releases).

**Supported platforms:**
- Linux (x64, arm64)
- macOS (x64, Apple Silicon)
- Windows (x64)

If the download fails (e.g. behind a corporate proxy), the installer falls back
to `go install github.com/Adhamzineldin/Veld/cmd/veld@latest`.

## Alternative installation

```bash
# Go (no npm needed)
go install github.com/Adhamzineldin/Veld/cmd/veld@latest

# Homebrew
brew install veld

# pip
pip install maayn-veld

# Composer (PHP)
composer require veld-dev/veld
```

## Links

- [Documentation](https://github.com/Adhamzineldin/Veld)
- [VS Code Extension](https://marketplace.visualstudio.com/items?itemName=veld-dev.veld-vscode)
- [JetBrains Plugin](https://plugins.jetbrains.com/plugin/veld)

## License

MIT

