#!/usr/bin/env bash
# sync-schema.sh — copies the canonical JSON schema to all editor plugins.
# Run this after editing editors/veld-config.schema.json.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CANONICAL="$ROOT/editors/veld-config.schema.json"

if [ ! -f "$CANONICAL" ]; then
  echo "ERROR: canonical schema not found at $CANONICAL" >&2
  exit 1
fi

# VS Code
cp "$CANONICAL" "$ROOT/editors/vscode/veld-config.schema.json"
echo "✓ synced → editors/vscode/veld-config.schema.json"

# JetBrains
cp "$CANONICAL" "$ROOT/editors/jetbrains/src/main/resources/schemas/veld-config.schema.json"
echo "✓ synced → editors/jetbrains/src/main/resources/schemas/veld-config.schema.json"

echo ""
echo "Done. All editor schemas are in sync."

