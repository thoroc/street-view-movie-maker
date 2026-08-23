#!/usr/bin/env bash
# Validates YAML frontmatter in .context/*.md files against
# .claude/skills/context-file/assets/schemas/context-frontmatter.schema.json.
# Usage: validate-context-frontmatter.sh <file> [<file> ...]
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
SCHEMA="$ROOT/.claude/skills/context-file/assets/schemas/context-frontmatter.schema.json"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

bun "$SCRIPT_DIR/validate-context-frontmatter.ts" "$SCHEMA" "$@"
