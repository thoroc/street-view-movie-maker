#!/usr/bin/env bash
set -euo pipefail

CHECK_MODE=false
if [ "${1:-}" = "--check" ]; then
  CHECK_MODE=true
fi

ROOT="$(git rev-parse --show-toplevel)"
INDEX="$ROOT/.context/index.yaml"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

bun "$SCRIPT_DIR/regenerate-context-index.ts" "$ROOT" "$INDEX" "$CHECK_MODE"
