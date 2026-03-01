# Veld — pip package

> Contract-first, multi-stack API code generator.

## Install

```bash
pip install veld
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

This pip package is a thin wrapper around the Veld Go binary. On first run,
it downloads the correct pre-built binary for your platform from
[GitHub Releases](https://github.com/veld-dev/veld/releases) and caches it
in your platform's cache directory.

**Supported platforms:**
- Linux (x64, arm64)
- macOS (x64, Apple Silicon)
- Windows (x64)

If the download fails, the installer falls back to
`go install github.com/veld-dev/veld/cmd/veld@latest`.

## Alternative installation

```bash
# npm
npm install veld

# Go
go install github.com/veld-dev/veld/cmd/veld@latest

# Homebrew
brew install veld

# Composer (PHP)
composer require veld-dev/veld
```

## License

MIT

