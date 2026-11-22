#!/usr/bin/env bash
# Fill the sha256 values of Formula/maayn-veld.rb from a release's SHA256SUMS file.
# An optional second argument names another formula file to update (used by the tests).
set -euo pipefail
sums="${1:?usage: update-formula.sh SHA256SUMS [formula.rb]}"
formula="${2:-$(dirname "$0")/Formula/maayn-veld.rb}"
for target in aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl; do
  sha="$(awk -v a="veld-$target.tar.gz" '$2 == a || $2 == "*" a { print $1 }' "$sums")"
  [ -n "$sha" ] || { echo "no checksum for $target" >&2; exit 1; }
  placeholder="PLACEHOLDER_SHA256_$(echo "$target" | tr 'a-z-' 'A-Z_')"
  sed -i.bak "s/$placeholder/$sha/" "$formula"
done
rm -f "$formula.bak"
