#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
INDEX="$ROOT/.context/index.yaml"
THRESHOLD_DAYS="${PLAN_STALENESS_THRESHOLD_DAYS:-60}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

bun "$SCRIPT_DIR/check-plan-staleness.ts" "$INDEX" "$THRESHOLD_DAYS"
