#!/usr/bin/env bash
set -euo pipefail

# Enforces the .context/ filename convention documented in
# .claude/skills/context-file/SKILL.md and references/context-file-types.md:
# every type is date-first, YYYY-MM-DD-<slug>.md, matching journal-entry
# convention. Also rejects any .md file living outside the canonical
# subdirectories (audits/ is exempt — it is owned by skill-auditor, not this
# schema).

ROOT="$(git rev-parse --show-toplevel)"

if [[ $# -gt 0 ]]; then
    files=("$@")
else
    files=()
    while IFS= read -r line; do
        files+=("$line")
    done < <(find "$ROOT/.context" -type f -name "*.md")
fi

date_first_re='^[0-9]{4}-[0-9]{2}-[0-9]{2}-[a-z0-9]+(-[a-z0-9]+)*\.md$'
allowed_dirs=("plans" "findings" "analysis" "follow-ups" "audits" "learnings" "handover")

errors=()

for f in "${files[@]}"; do
    [[ -f "$f" ]] || continue
    case "$f" in
        *.context/*.md) ;;
        *) continue ;;
    esac

    rel="${f#"$ROOT"/}"
    subdir="${rel#.context/}"
    subdir="${subdir%%/*}"
    base="$(basename "$f")"

    known=false
    for d in "${allowed_dirs[@]}"; do
        if [[ "$subdir" == "$d" ]]; then
            known=true
            break
        fi
    done
    if [[ "$known" == false ]]; then
        errors+=("$rel: lives in .context/$subdir/, not one of plans/findings/analysis/follow-ups/learnings/handover")
        continue
    fi
    [[ "$subdir" == "audits" ]] && continue

    if [[ ! "$base" =~ $date_first_re ]]; then
        errors+=("$rel: expected YYYY-MM-DD-slug.md (date-first) for $subdir/")
        continue
    fi
    fn_date="${base:0:10}"

    fm_date=$(grep -m1 '^date:' "$f" | sed -E 's/^date: *"?([0-9-]+)"?.*/\1/')
    if [[ -n "$fm_date" && "$fm_date" != "$fn_date" ]]; then
        errors+=("$rel: filename date ($fn_date) does not match frontmatter date ($fm_date)")
    fi
done

if [[ ${#errors[@]} -gt 0 ]]; then
    echo "ERROR: .context/ filenames violate naming convention:"
    for e in "${errors[@]}"; do
        echo "  $e"
    done
    printf '\nSee .claude/skills/context-file/SKILL.md workflow step 2 and references/context-file-types.md for the naming rule.\n'
    exit 1
fi

echo "context filenames OK"
