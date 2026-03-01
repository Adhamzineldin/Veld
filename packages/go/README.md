# Veld — Go package

> Contract-first, multi-stack API code generator.

## Install

```bash
go install github.com/Adhamzineldin/Veld/cmd/veld@latest
```

This downloads, builds, and installs the `veld` binary to your `$GOPATH/bin`.
Make sure `$GOPATH/bin` is in your `PATH`.

## Usage

```bash
veld init                    # Scaffold a new project
veld generate                # Generate from veld.config.json
veld generate --dry-run      # Preview what would be generated
veld watch                   # Auto-regenerate on file changes
veld validate                # Check contracts for errors
veld clean                   # Remove generated output
veld openapi                 # Export OpenAPI 3.0 spec
```

## Build from source

```bash
git clone https://github.com/Adhamzineldin/Veld.git
cd veld
go build -o veld ./cmd/veld
```

## Alternative installation

```bash
# npm
npm install @maayn/veld

# pip
pip install maayn-veld

# Homebrew
brew install veld

# Composer (PHP)
composer require veld-dev/veld
```

## License

MIT

