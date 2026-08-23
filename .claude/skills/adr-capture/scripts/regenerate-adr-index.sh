#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
ADR_DIR="$ROOT/docs/ADR"
INDEX="$ADR_DIR/index.yaml"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

bun "$SCRIPT_DIR/regenerate-adr-index.ts" "$ADR_DIR" "$INDEX"
