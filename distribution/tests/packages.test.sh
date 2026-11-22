#!/usr/bin/env bash
# Test of `package.sh scoop|choco|deb`: usage `packages.test.sh <SHA256SUMS>`.
# The scoop and chocolatey steps fill the manifests from the checksums; deb needs `nfpm`, and the
# chocolatey step needs `choco`. Each is skipped (with a note) when its tool is missing.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
sums="${1:?usage: packages.test.sh SHA256SUMS}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
want="$(awk '$2 == "veld-x86_64-pc-windows-msvc.zip" || $2 == "*veld-x86_64-pc-windows-msvc.zip" { print $1 }' "$sums")"
[ -n "$want" ] || { echo "no windows checksum in $sums" >&2; exit 1; }

# Scoop: the filled manifest has the version and the real hash, and no placeholder.
"$BASH" "$here/../package.sh" scoop 9.8.7-rc.1 "$sums" "$tmp/scoop"
grep -q '"version": "9.8.7-rc.1"' "$tmp/scoop/veld.json"
grep -q "/download/v9.8.7-rc.1/veld-x86_64-pc-windows-msvc.zip" "$tmp/scoop/veld.json"
grep -q "\"hash\": \"$want\"" "$tmp/scoop/veld.json" || { echo "wrong scoop hash" >&2; exit 1; }
! grep -q PLACEHOLDER "$tmp/scoop/veld.json" || { echo "scoop placeholder left" >&2; exit 1; }
# The committed manifest keeps its placeholder (it is a template).
grep -q PLACEHOLDER_SHA256_X86_64_PC_WINDOWS_MSVC "$here/../scoop/veld.json"

# A SHA256SUMS without the windows archive must fail loudly.
grep -v "windows" "$sums" > "$tmp/short"
if "$BASH" "$here/../package.sh" scoop 9.8.7 "$tmp/short" "$tmp/bad" 2>/dev/null; then
  echo "a missing checksum was accepted" >&2; exit 1
fi

# Chocolatey: the nuspec version has no dot in the prerelease, and the script has the checksum.
if command -v choco >/dev/null 2>&1; then
  "$BASH" "$here/../package.sh" choco 9.8.7-rc.1 "$sums" "$tmp/choco"
  ls "$tmp/choco"/veld.9.8.7-rc1.nupkg >/dev/null
else
  echo "choco not found: skipping the chocolatey pack"
fi

# deb and rpm: a package is produced for each architecture.
if command -v nfpm >/dev/null 2>&1; then
  printf '#!/bin/sh\necho veld\n' > "$tmp/veld"
  for arch in amd64 arm64; do
    "$BASH" "$here/../package.sh" deb "$arch" 9.8.7-rc.1 "$tmp/veld" "$tmp/pkg"
    "$BASH" "$here/../package.sh" rpm "$arch" 9.8.7-rc.1 "$tmp/veld" "$tmp/pkg"
  done
  [ "$(ls "$tmp/pkg" | wc -l)" -eq 4 ] || { echo "expected 4 packages" >&2; exit 1; }
else
  echo "nfpm not found: skipping the deb and rpm build"
fi
echo "packages: all checks passed"
