# Veld — Composer package

> Contract-first, multi-stack API code generator.

## Install

```bash
composer require veld-dev/veld
```

## Usage

After installing, the `veld` CLI is available via Composer's bin directory:

```bash
vendor/bin/veld init              # Scaffold a new project
vendor/bin/veld generate          # Generate from veld.config.json
vendor/bin/veld watch             # Auto-regenerate on file changes
vendor/bin/veld validate          # Check contracts for errors
vendor/bin/veld openapi           # Export OpenAPI 3.0 spec
```

Or if you have `vendor/bin` in your PATH:

```bash
veld generate
```

## How it works

This Composer package is a thin PHP wrapper around the Veld Go binary. On first
run, it downloads the correct pre-built binary for your platform from
[GitHub Releases](https://github.com/Adhamzineldin/Veld/releases) and caches it.

**Supported platforms:**
- Linux (x64, arm64)
- macOS (x64, Apple Silicon)
- Windows (x64)

If the download fails, the wrapper falls back to
`go install github.com/Adhamzineldin/Veld/cmd/veld@latest`.

## Alternative installation

```bash
# npm
npm install @maayn/veld

# pip
pip install maayn-veld

# Go
go install github.com/Adhamzineldin/Veld/cmd/veld@latest

# Homebrew
brew install veld
```

## License

MIT

