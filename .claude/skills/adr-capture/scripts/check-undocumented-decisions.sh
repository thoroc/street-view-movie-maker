#!/usr/bin/env bash
# Scans .context/**/*.md for decision indicators and cross-references
# against ADR context: links. Reports any context files that appear
# to contain decisions but are not documented as an ADR.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
CONTEXT_DIR="$ROOT/.context"
ADR_DIR="$ROOT/docs/ADR"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

bun "$SCRIPT_DIR/check-undocumented-decisions.ts" "$CONTEXT_DIR" "$ADR_DIR"
