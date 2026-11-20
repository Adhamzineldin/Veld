#!/usr/bin/env bash
# Test of homebrew/update-formula.sh: usage `homebrew-formula.test.sh <SHA256SUMS>`.
# Works on a temporary copy of the formula; the committed one keeps its placeholders.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
sums="${1:?usage: homebrew-formula.test.sh SHA256SUMS}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cp "$here/../homebrew/Formula/maayn-veld.rb" "$tmp/f.rb"

bash "$here/../homebrew/update-formula.sh" "$sums" "$tmp/f.rb"
! grep -q PLACEHOLDER_SHA256 "$tmp/f.rb" || { echo "placeholders left" >&2; exit 1; }
for target in aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl; do
  want="$(awk -v a="veld-$target.tar.gz" '$2 == a || $2 == "*" a { print $1 }' "$sums")"
  grep -q "veld-$target.tar.gz\"" "$tmp/f.rb"
  grep -A1 "veld-$target.tar.gz\"" "$tmp/f.rb" | grep -q "sha256 \"$want\"" || { echo "wrong sha for $target" >&2; exit 1; }
done

# A SHA256SUMS without one of the assets must fail loudly and change nothing.
grep -v "aarch64-apple-darwin" "$sums" > "$tmp/short"
cp "$here/../homebrew/Formula/maayn-veld.rb" "$tmp/g.rb"
if bash "$here/../homebrew/update-formula.sh" "$tmp/short" "$tmp/g.rb" 2>/dev/null; then
  echo "a missing checksum was accepted" >&2; exit 1
fi
echo "homebrew formula: all checks passed"
