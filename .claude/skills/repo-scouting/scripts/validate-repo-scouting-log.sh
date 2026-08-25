#!/usr/bin/env bash
# Validates every line of .context/repo-scouting/log.jsonl against
# assets/schemas/repo-scouting-entry.schema.json. Constraints (the verdict
# enum, url/date patterns, required fields) are read from the schema file at
# runtime via jq, not duplicated as separate hardcoded values here -- see
# .claude/instructions/skill-authoring.md for why: a hand-duplicated copy
# silently drifts from the schema the moment one changes without the other.
#
# Two independent checks, per the plan this skill was built from
# (.context/plans/2026-08-25-add-repo-scouting-log-skill.md):
#   1. Schema conformance -- every line is valid JSON matching the schema.
#   2. Duplicate `url` values, compared case-insensitively (GitHub URLs
#      aren't case-sensitive in practice) -- the log's entire purpose is
#      "have we checked this before," so a duplicate silently defeats it.
# `related` paths are checked for existence but only WARN, never fail --
# a path can legitimately point at a finding/plan not written yet.
#
# Usage: validate-repo-scouting-log.sh [<file>]  (defaults to .context/repo-scouting/log.jsonl)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA="$SCRIPT_DIR/../assets/schemas/repo-scouting-entry.schema.json"
FILE="${1:-.context/repo-scouting/log.jsonl}"

if [ ! -f "$SCHEMA" ]; then
  echo "validate-repo-scouting-log.sh: $SCHEMA not found" >&2
  exit 1
fi

if [ ! -f "$FILE" ]; then
  echo "validate-repo-scouting-log.sh: $FILE not found (nothing logged yet)" >&2
  exit 1
fi

# Sanity check that this script's known field list still matches the schema's
# required list -- a tripwire for exactly the drift skill-authoring.md warns
# against.
expected_fields="url,date,repo_name,summary,verdict,reasoning"
required_fields="$(jq -r '.required | join(",")' "$SCHEMA")"
if [ "$required_fields" != "$expected_fields" ]; then
  echo "validate-repo-scouting-log.sh: schema required fields ($required_fields) no longer" >&2
  echo "match this script's known field list ($expected_fields) -- update the script to match." >&2
  exit 1
fi

verdict_enum_json="$(jq -c '.properties.verdict.enum' "$SCHEMA")"
url_pattern="$(jq -r '.properties.url.pattern' "$SCHEMA")"
date_pattern="$(jq -r '.properties.date.pattern' "$SCHEMA")"
allowed_keys_json="$(jq -c '.properties | keys' "$SCHEMA")"

# jq -r prints the literal string "null" (exit 0) for a path that doesn't
# exist -- it does not error or leave the variable unset, so a
# renamed/removed pattern key beneath .properties (not caught by the
# .required tripwire above, which only checks field *names*) would otherwise
# silently validate every entry against the string "null" instead of failing
# loudly.
for name in url_pattern date_pattern; do
  if [ "${!name}" = "null" ]; then
    echo "validate-repo-scouting-log.sh: $SCHEMA is missing a property this script reads" >&2
    echo "($name resolved to jq's \"null\") -- update the script or the schema to match." >&2
    exit 1
  fi
done

log_dir="$(cd "$(dirname "$FILE")" && pwd)"
errors=0
warnings=0
lines_checked=0
declare -A seen_urls

while IFS= read -r line; do
  [ -z "$line" ] && continue
  lines_checked=$((lines_checked + 1))
  label="line $lines_checked"

  if ! entry="$(jq -ce '.' <<<"$line" 2>/dev/null)"; then
    echo "$FILE: $label: not valid JSON" >&2
    errors=$((errors + 1))
    continue
  fi

  # additionalProperties: false -- flag any key not declared in the schema.
  extra_keys="$(jq -r --argjson allowed "$allowed_keys_json" \
    'keys - $allowed | .[]' <<<"$entry")"
  if [ -n "$extra_keys" ]; then
    echo "$FILE: $label: unexpected field(s) not in schema: $(tr '\n' ' ' <<<"$extra_keys")" >&2
    errors=$((errors + 1))
  fi

  # For each required field, missing or present-but-empty both count as
  # "missing" -- `$entry[.] // ""` turns an absent key into "" so both cases
  # collapse to the same length==0 check.
  missing_required="$(jq -r --argjson req "$(jq -c '.required' "$SCHEMA")" \
    '. as $entry | $req[] | select(($entry[.] // "") | tostring | length == 0)' <<<"$entry")"
  if [ -n "$missing_required" ]; then
    echo "$FILE: $label: missing or empty required field(s): $(tr '\n' ' ' <<<"$missing_required")" >&2
    errors=$((errors + 1))
    continue
  fi

  url="$(jq -r '.url' <<<"$entry")"
  date="$(jq -r '.date' <<<"$entry")"
  verdict="$(jq -r '.verdict' <<<"$entry")"

  if ! [[ "$url" =~ $url_pattern ]]; then
    echo "$FILE: $label: url '$url' does not match $url_pattern" >&2
    errors=$((errors + 1))
  fi
  if ! [[ "$date" =~ $date_pattern ]]; then
    echo "$FILE: $label: date '$date' does not match $date_pattern" >&2
    errors=$((errors + 1))
  fi
  if ! jq -e --arg v "$verdict" --argjson enum "$verdict_enum_json" \
    '$enum | index($v) != null' >/dev/null <<<null; then
    echo "$FILE: $label: verdict '$verdict' is not one of $(jq -r 'join(", ")' <<<"$verdict_enum_json")" >&2
    errors=$((errors + 1))
  fi

  url_lower="$(tr '[:upper:]' '[:lower:]' <<<"$url")"
  url_lower="${url_lower%/}"
  if [ -n "${seen_urls[$url_lower]+x}" ]; then
    echo "$FILE: $label: duplicate url '$url' (first seen at ${seen_urls[$url_lower]})" >&2
    errors=$((errors + 1))
  else
    seen_urls[$url_lower]="$label"
  fi

  # related paths: warn only, never fail (see script header).
  related_paths="$(jq -r 'select(has("related")) | .related[]?' <<<"$entry" 2>/dev/null || true)"
  if [ -n "$related_paths" ]; then
    while IFS= read -r rel_path; do
      [ -z "$rel_path" ] && continue
      if [ ! -e "$log_dir/$rel_path" ]; then
        echo "$FILE: $label: WARNING: related path '$rel_path' does not exist (resolved: $log_dir/$rel_path)" >&2
        warnings=$((warnings + 1))
      fi
    done <<<"$related_paths"
  fi
done < "$FILE"

if [ "$errors" -gt 0 ]; then
  echo "" >&2
  echo "$errors schema violation(s) found in $FILE ($warnings warning(s))" >&2
  exit 1
fi

echo "ok -- all $lines_checked entry(ies) in $FILE match repo-scouting-entry.schema.json ($warnings warning(s))"
