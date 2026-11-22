#!/usr/bin/env bash
# Builds the release archives with the exact names the wrappers download. release.yml and local
# runs both call this script, so a local build is byte-for-byte the same packaging as CI.
#
#   package.sh veld     <target-triple> <path-to-veld-binary> <out-dir>   veld-<target>.tar.gz|zip
#   package.sh registry <amd64|arm64>   <path-to-binary>      <out-dir>   veld-registry-linux-<arch>.tar.gz
#   package.sh sums     <dir>                                             <dir>/SHA256SUMS
#   package.sh deb|rpm  <amd64|arm64> <version> <path-to-binary> <out-dir>   veld_<version>_<arch>.deb (needs nfpm)
#   package.sh choco    <version> <SHA256SUMS> <out-dir>                     veld.<version>.nupkg (needs choco)
#   package.sh scoop    <version> <SHA256SUMS> <out-dir>                     <out-dir>/veld.json
#
# Each veld archive holds the binary (veld, or veld.exe on Windows) at its root.
set -euo pipefail

sum_for() { # sum_for <SHA256SUMS> <archive-name>
  local sha
  sha="$(awk -v a="$2" '$2 == a || $2 == "*" a { print $1 }' "$1")"
  [ -n "$sha" ] || { echo "no checksum for $2 in $1" >&2; exit 1; }
  printf '%s' "$sha"
}

usage() { sed -n '2,15p' "$0" >&2; exit 2; }

zip_dir() { # zip_dir <dir-with-files> <archive.zip>
  local dir="$1" out="$2"
  rm -f "$out"
  if command -v zip >/dev/null 2>&1; then
    (cd "$dir" && zip -q -r "$out" .)
  elif command -v 7z >/dev/null 2>&1; then
    (cd "$dir" && 7z a -tzip -bso0 "$out" .)
  else
    if command -v cygpath >/dev/null 2>&1; then dir="$(cygpath -w "$dir")"; out="$(cygpath -w "$out")"; fi
    powershell -NoProfile -Command "Compress-Archive -Path '$dir\*' -DestinationPath '$out'"
  fi
}

cmd="${1:-}"
case "$cmd" in
  veld)
    target="${2:?target}"; bin="${3:?binary}"; out="${4:?out-dir}"
    mkdir -p "$out"; out="$(cd "$out" && pwd)"
    name="veld-$target"
    stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
    case "$target" in
      *windows*) cp "$bin" "$stage/veld.exe"; zip_dir "$stage" "$out/$name.zip" ;;
      *) cp "$bin" "$stage/veld"; chmod 755 "$stage/veld"; tar -C "$stage" -czf "$out/$name.tar.gz" . ;;
    esac
    ;;
  registry)
    arch="${2:?goarch}"; bin="${3:?binary}"; out="${4:?out-dir}"
    mkdir -p "$out"; out="$(cd "$out" && pwd)"
    stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
    cp "$bin" "$stage/veld-registry"; chmod 755 "$stage/veld-registry"
    tar -C "$stage" -czf "$out/veld-registry-linux-$arch.tar.gz" veld-registry
    ;;
  sums)
    dir="${2:?dir}"
    cd "$dir"
    # One line per archive: "<sha256>  <name>", sorted by name, as `sha256sum` writes them.
    if command -v sha256sum >/dev/null 2>&1; then
      LC_ALL=C sha256sum --text $(ls *.tar.gz *.zip 2>/dev/null | LC_ALL=C sort) > SHA256SUMS
    else
      LC_ALL=C shasum -a 256 $(ls *.tar.gz *.zip 2>/dev/null | LC_ALL=C sort) > SHA256SUMS
    fi
    cat SHA256SUMS
    ;;
  deb|rpm)
    # package.sh deb|rpm <amd64|arm64> <version> <path-to-veld-binary> <out-dir>   needs `nfpm` on PATH
    arch="${2:?arch (amd64|arm64)}"; version="${3:?version}"; bin="${4:?binary}"; out="${5:?out-dir}"
    mkdir -p "$out"; out="$(cd "$out" && pwd)"
    here="$(cd "$(dirname "$0")" && pwd)"
    ARCH="$arch" VERSION="$version" VELD_BIN="$bin" VELD_LICENSE="$here/npm/LICENSE" \
      nfpm pkg --config "$here/apt/nfpm.yaml" --packager "$cmd" --target "$out/"
    ;;
  choco)
    # package.sh choco <version> <SHA256SUMS> <out-dir>   needs `choco` (Windows); writes veld.<version>.nupkg
    version="${2:?release version, for example 2.0.0-beta.1}"; sums="${3:?SHA256SUMS}"; out="${4:?out-dir}"
    mkdir -p "$out"; out="$(cd "$out" && pwd)"
    here="$(cd "$(dirname "$0")" && pwd)"
    stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
    cp -r "$here/chocolatey/." "$stage/"
    sha="$(sum_for "$sums" veld-x86_64-pc-windows-msvc.zip)"
    # Chocolatey prerelease identifiers allow no dots: 2.0.0-beta.1 becomes 2.0.0-beta1.
    choco_version="$(printf '%s' "$version" | sed -E 's/^([0-9]+\.[0-9]+\.[0-9]+-[A-Za-z]+)\.([0-9]+)$/\1\2/')"
    sed -i.bak "s|<version>.*</version>|<version>$choco_version</version>|" "$stage/veld.nuspec"
    sed -i.bak "s|^\$releaseVersion = .*|\$releaseVersion = '$version'|; s|PLACEHOLDER_SHA256_X86_64_PC_WINDOWS_MSVC|$sha|" "$stage/tools/chocolateyinstall.ps1"
    rm -f "$stage"/veld.nuspec.bak "$stage"/tools/*.bak
    (cd "$stage" && choco pack veld.nuspec --out "$(if command -v cygpath >/dev/null 2>&1; then cygpath -w "$out"; else echo "$out"; fi)")
    ;;
  scoop)
    # package.sh scoop <version> <SHA256SUMS> <out-dir>   writes the filled manifest <out-dir>/veld.json
    version="${2:?release version}"; sums="${3:?SHA256SUMS}"; out="${4:?out-dir}"
    mkdir -p "$out"
    here="$(cd "$(dirname "$0")" && pwd)"
    sha="$(sum_for "$sums" veld-x86_64-pc-windows-msvc.zip)"
    sed -E "s|\"version\": \"[^\"]*\"|\"version\": \"$version\"|; s|/download/v[^/]*/veld-x86_64-pc-windows-msvc.zip\"|/download/v$version/veld-x86_64-pc-windows-msvc.zip\"|; s|PLACEHOLDER_SHA256_X86_64_PC_WINDOWS_MSVC|$sha|" \
      "$here/scoop/veld.json" > "$out/veld.json"
    ;;
  *) usage ;;
esac
