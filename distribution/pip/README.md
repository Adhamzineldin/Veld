# Veld - pip package

> Contract-first API code generator. Write `.veld` contracts once, generate typed servers and clients.

## Install

```bash
pip install maayn-veld
```

On first run the wrapper downloads the `veld` binary that matches this package's version from the
[GitHub release](https://github.com/Adhamzineldin/Veld/releases), verifies it against the release's
`SHA256SUMS`, and caches it under `~/.veld/bin/<version>/` (`VELD_HOME` changes the root). A checksum
mismatch aborts. `VELD_VERSION` overrides the version.

Supported platforms: Linux (x64, arm64), macOS (x64, arm64), Windows (x64).

## Usage

```bash
veld init       # create veld.toml and an example contract
veld check      # compile the contracts and report diagnostics
veld generate   # generate code for every target
veld clean      # remove generated files you have not modified
```

## From source

```bash
cargo install --git https://github.com/Adhamzineldin/Veld veld
```

## License

MIT
