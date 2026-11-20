# Veld — npm package

> Contract-first API code generator. Write `.veld` contracts once, generate typed servers and clients.

## Install

```bash
npm install -g @maayn/veld
# or
npx @maayn/veld generate
```

The `postinstall` script downloads the `veld` binary that matches this package's version from the
[GitHub release](https://github.com/Adhamzineldin/Veld/releases), verifies it against the release's
`SHA256SUMS`, and unpacks it. A checksum mismatch aborts the install.

Supported platforms: Linux (x64, arm64), macOS (x64, arm64), Windows (x64).
Set `VELD_VERSION` to override the version to download.

## Usage

```bash
veld init       # create veld.toml and an example contract
veld check      # compile the contracts and report diagnostics
veld generate   # generate code for every target
veld clean      # remove generated files you have not modified
```

See the [main README](https://github.com/Adhamzineldin/Veld#readme) for the full command list.

## From source

```bash
cargo install --git https://github.com/Adhamzineldin/Veld veld
```

## License

MIT
