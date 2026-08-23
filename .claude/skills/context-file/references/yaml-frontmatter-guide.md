# YAML Frontmatter Guide

This reference covers the YAML frontmatter schema used by all `.context/` files.

## Required Fields

```yaml
---
title: "Human-readable title"
type: plan | finding | analysis | follow-up | learning | handover
status: draft | active | done | superseded
date: YYYY-MM-DD
---
```

| Field | Type | Rules |
|-------|------|-------|
| `title` | string | Prose title matching the H1 heading; wrap in quotes |
| `type` | enum | One of: `plan`, `finding`, `analysis`, `follow-up`, `learning`, `handover` — must match the subdirectory |
| `status` | enum | `draft` until reviewed, `active` for in-progress, `done` when complete, `superseded` when replaced |
| `date` | string (date) | ISO 8601 date `YYYY-MM-DD` — set once on creation, never updated |

## Optional Fields

### `related`

List of relative paths to related `.context/` files. The path is relative to the file's own location.

```yaml
related:
  - ../plans/related-plan.md
  - ../findings/related-finding-2026-06-30.md
```

**Rules:**
- Omit the key entirely when there are no related files
- Never use `related: []` (empty list) — either omit or populate
- Use relative paths with `../` to navigate between subdirectories

### Examples by Type

**Plan** (`.context/plans/`):
```yaml
---
title: "Plan: Add Structured Logging"
type: plan
status: draft
date: 2026-06-30
---
```

**Finding** (`.context/findings/`):
```yaml
---
title: "Finding: Logging Library Evaluation"
type: finding
status: active
date: 2026-06-30
related:
  - ../plans/add-structured-logging.md
---
```

**Analysis** (`.context/analysis/`):
```yaml
---
title: "CLI Flag Audit — 2026-06-30"
type: analysis
status: done
date: 2026-06-30
---
```

**Learning** (`.context/learnings/`):
```yaml
---
title: "Regenerate mise.lock when mise.toml changes"
type: learning
status: active
date: 2026-08-17
---
```

**Handover** (`.context/handover/`):
```yaml
---
title: "2026-08-16-ci-workflow-port session handover"
type: handover
status: done
date: 2026-08-16
---
```

## Validation

Run `.claude/skills/context-index/check-context-frontmatter.sh .context/**/*.md` to validate all files.
