# Veld — Homebrew

> Contract-first, multi-stack API code generator.

## Install

```bash
# Add the tap (one-time)
brew tap veld-dev/tap

# Install
brew install veld
```

Or in one command:
```bash
brew install veld-dev/tap/veld
```

## Usage

```bash
veld init                    # Scaffold a new project
veld generate                # Generate from veld.config.json
veld watch                   # Auto-regenerate on file changes
veld validate                # Check contracts for errors
veld openapi                 # Export OpenAPI 3.0 spec
```

## Updating

```bash
brew upgrade veld
```

## Publishing a new version

1. Build binaries for all platforms (via CI or manually):
   - `veld-darwin-amd64.tar.gz`
   - `veld-darwin-arm64.tar.gz`
   - `veld-linux-amd64.tar.gz`
   - `veld-linux-arm64.tar.gz`

2. Upload to GitHub Releases as `v{VERSION}`

3. Compute SHA256 for each archive:
   ```bash
   shasum -a 256 veld-*.tar.gz
   ```

4. Update `Formula/veld.rb` with new version, URLs, and SHA256 values

5. Push to the `homebrew-tap` repository

## License

MIT

