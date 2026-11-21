#!/usr/bin/env bash
# Builds the release archives with the exact names the wrappers download. release.yml and local
# runs both call this script, so a local build is byte-for-byte the same packaging as CI.
#
#   package.sh veld     <target-triple> <path-to-veld-binary> <out-dir>   veld-<target>.tar.gz|zip
#   package.sh registry <amd64|arm64>   <path-to-binary>      <out-dir>   veld-registry-linux-<arch>.tar.gz
#   package.sh sums     <dir>                                             <dir>/SHA256SUMS
#
# Each veld archive holds the binary (veld, or veld.exe on Windows) at its root.
set -euo pipefail

usage() { sed -n '2,9p' "$0" >&2; exit 2; }

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
  *) usage ;;
esac
