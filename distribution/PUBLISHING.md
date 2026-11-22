# Publishing

Publish only after the release workflow has uploaded the assets for the tag, because the wrappers
download them. The version in `npm/package.json`, `pip/pyproject.toml`, `pip/veld/__init__.py` and
`homebrew/Formula/maayn-veld.rb` must equal the tag without the `v`.

## npm

```bash
cd distribution/npm
npm publish --access public      # --tag next for a pre-release
```

## PyPI

```bash
cd distribution/pip
pip install build twine
python -m build
twine upload dist/*
```

## Homebrew

```bash
cd distribution/homebrew
./update-formula.sh /path/to/SHA256SUMS
# copy Formula/maayn-veld.rb into the homebrew-tap repository and push
```

## apt and rpm (.deb, .rpm)

Built by the release workflow with [nfpm](https://nfpm.goreleaser.com) (`distribution/apt/nfpm.yaml`)
from the static Linux archives and attached to the GitHub release as
`veld_<version>_<amd64|arm64>.deb` and `veld-<version>-1.<x86_64|aarch64>.rpm`. A prerelease
`2.0.0-beta.1` is versioned `2.0.0~beta.1`, so it sorts before `2.0.0`. Locally:

```bash
bash distribution/package.sh deb amd64 2.0.0-beta.1 path/to/veld dist   # needs nfpm
```

To serve them from an apt or yum repository, upload the files to your repository tool
(aptly, reprepro, createrepo). The release only produces the packages.

## Chocolatey

`distribution/chocolatey` is the package source: `veld.nuspec` and `tools/chocolateyinstall.ps1`,
which downloads the release zip and verifies its SHA256. The workflow fills the version and the
checksum from `SHA256SUMS` and runs `choco pack`. To publish:

```powershell
choco apikey --key <key> --source https://push.chocolatey.org/
choco push veld.2.0.0-beta1.nupkg --source https://push.chocolatey.org/
```

Chocolatey prerelease versions cannot contain dots, so `2.0.0-beta.1` is package version
`2.0.0-beta1`; the script still downloads release `v2.0.0-beta.1`.

## Scoop

`distribution/scoop/veld.json` is a template (the hash is a placeholder). The workflow attaches the
filled manifest as the release asset `veld.json`. To publish, add it to a bucket repository:

```powershell
scoop bucket add maayn https://github.com/Adhamzineldin/scoop-bucket
scoop install maayn/veld
```

The manifest has `checkver` and `autoupdate` (hash read from the release's `SHA256SUMS`), so a bucket
that runs Scoop's excavator updates itself.
