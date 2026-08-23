#!/usr/bin/env bash
# Validates YAML frontmatter in docs/ADR/adr-*.md files against
# .claude/skills/adr-capture/assets/schemas/adr-frontmatter.schema.json.
# Usage: validate-adr-frontmatter.sh <file> [<file> ...]
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
SCHEMA="$ROOT/.claude/skills/adr-capture/assets/schemas/adr-frontmatter.schema.json"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

bun "$SCRIPT_DIR/validate-adr-frontmatter.ts" "$SCHEMA" "$@"
