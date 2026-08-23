---
name: context-file
description:
  "Create a new .context/ file (plan, finding, analysis, or follow-up) with standard YAML frontmatter. Use when documenting a decision, writing an implementation plan, recording research findings,
  capturing analysis output, or logging deferred/outstanding work. DO NOT use for ephemeral notes, secrets storage, or skill remediation plans (use skill-auditor remediate instead). Triggers: 'create
  a plan', 'new finding', 'document this', 'write analysis', 'new context file', 'capture findings', 'draft a plan', 'record decision', 'log a follow-up', 'track outstanding work'."
---

# Context File

Create a new `.context/` file with standard YAML frontmatter and appropriate sections.

## Prerequisites

- A clear understanding of the file type needed: plan (multi-step work), finding (research), analysis (review), follow-up (deferred work), learning (distilled rules from past sessions), or handover (session hand-off state)
- Familiarity with the `.context/` directory structure: `plans/`, `findings/`, `analysis/`, `follow-ups/`, `learnings/`, `handover/`
- The `context-index` skill available for index regeneration after creation

## When to Use

- **plan**: Multi-step implementation, migration, or remediation work with open tasks
- **finding**: Research output, code review results, audit findings, prerequisite investigations
- **analysis**: Duplication reports, benchmark results, comparative reviews, one-off audits
- **follow-up**: Deferred work or open threads surfaced mid-session but out of scope for the current change — file it the same turn it's identified, do not just mention it in the response and move on
- **learning**: Distilled rules or lessons from past sessions — standing guidance until superseded
- **handover**: Session hand-off state for the next session to resume without re-discovery

## When Not to Use

- For skill remediation plans, use `skill-auditor remediate` — it produces a richer schema
- Do not store secrets, credentials, or personal data in `.context/` files
- Do not create `.context/` files for ephemeral notes — use inline comments instead

## Frontmatter Schema

Every `.context/` file MUST start with this exact block:

```yaml
---
title: "Human-readable title"
type: plan | finding | analysis | follow-up | learning | handover
status: draft | active | done | superseded
date: YYYY-MM-DD
related:
  - relative/path/to/related.md
---
```

Field rules:

- `title` — prose title matching the H1 heading; wrap in quotes
- `type` — matches the subdirectory (`plans/` → `plan`, `findings/` → `finding`, `analysis/` → `analysis`, `follow-ups/` → `follow-up`, `learnings/` → `learning`, `handover/` → `handover`)
- `status` — `draft` until reviewed, `active` for in-progress work, `done` when complete, `superseded` when replaced. For `follow-up`, `active` means still outstanding and `done` means actioned — flip
  it to `done` in the same change that resolves it, never delete the file
- `date` — creation date in ISO format; do not update on edits
- `related` — relative paths from the file's location; omit the key entirely if there are no related files

## Workflow

1. Determine type: plan / finding / analysis / follow-up / learning / handover
2. Choose a filename: every `.context/` type is date-first, `YYYY-MM-DD-<slug>.md` (e.g. `2026-06-30-migrate-off-tessl-eval.md`) — same convention as journal entries
3. Create the file using the template matching the type below
4. Set `status: draft` until the content is reviewed (follow-ups start at `active` — they are already actionable, not draft)
5. Run the context index regeneration script to update the index after creation

## Templates

**Plan** (`.context/plans/`):

```markdown
---
title: "Plan: <concise title>"
type: plan
status: draft
date: YYYY-MM-DD
---

# Plan: <title>

## Goal

One paragraph describing the desired end state.

## Steps

1. Step one
2. Step two

## Open Questions

- Question one
```

**Finding** (`.context/findings/`):

```markdown
---
title: "Finding: <topic>"
type: finding
status: active
date: YYYY-MM-DD
related:
  - ../plans/related-plan.md
---

# Finding: <topic>

> One-sentence summary.

## Summary

## Detail

## Recommended Action
```

**Analysis** (`.context/analysis/`):

```markdown
---
title: "<Topic> Analysis — YYYY-MM-DD"
type: analysis
status: done
date: YYYY-MM-DD
---

# <Topic> Analysis — YYYY-MM-DD

## Summary

## Findings

## Conclusion
```

**Follow-up** (`.context/follow-ups/`):

```markdown
---
title: "Follow-up: <concise description>"
type: follow-up
status: active
date: YYYY-MM-DD
related:
  - ../../2026/07/2026-07-28-source-session.md
---

# Follow-up: <concise description>

## Context

One paragraph: what session/change surfaced this, and why it was out of scope then.

## Outstanding Work

- What needs to happen

## Action

Set `status: done` in this file (do not delete it) once actioned.
```

**Learning** (`.context/learnings/`):

```markdown
---
title: "<concise rule or lesson>"
type: learning
status: active
date: YYYY-MM-DD
---

# <concise rule or lesson>

## Learning

The rule, stated as one actionable sentence.

## Evidence

What happened, when, and why this rule emerged (commits/PRs/incidents).

## Rules

- Bullet-list the standing rules that follow.
```

**Handover** (`.context/handover/`):

```markdown
---
title: "<YYYY-MM-DD>-<slug> session handover"
type: handover
status: active
date: YYYY-MM-DD
---

# Handover — <slug>

## Current state

## What changed (commit → item)

## Key facts for the next session

## Next steps / what remains

Set `status: done` once the next session closes the handover.
```

## Mindset

- Write for the next agent or human who reads this cold — assume no prior context
- `status: draft` is the safe default; promote to `active` only when reviewed
- Date is creation date, not last-modified — do not update it on subsequent edits
- After creating or updating a `.context/` file, consider regenerating the index to keep it current
- Use production-grade terminology: pitfall, gotcha, ALWAYS, NEVER, anti-pattern

## Troubleshooting

- **Pre-commit hook blocks commit:** Run `check-context-frontmatter.sh` to find files missing YAML frontmatter — add the required block and re-run
- **Missing from index after creation:** Run `regenerate-context-index.sh` — the index is generated from frontmatter, not file existence alone
- **Wrong type directory:** Files must live under the matching subdirectory — plans in `plans/`, findings in `findings/`, analysis in `analysis/`, follow-ups in `follow-ups/`, learnings in `learnings/`, handovers in `handover/`
- **Wrong filename convention:** Run `check-context-filenames.sh` — it enforces date-first (`YYYY-MM-DD-slug.md`) across every `.context/` subdirectory and rejects files outside the canonical
  ones; wired into `hk.pkl` as the `context-filenames` step so it runs on every commit

## Anti-Patterns

**NEVER** create a `.context/` file without the full frontmatter block. **WHY:** The pre-commit hook will block the commit and the index cannot include the file. **BAD:** Starting a file with
`# Plan: ...` directly, with no frontmatter. **GOOD:** Always open with `---\ntitle: ...\ntype: ...\nstatus: ...\ndate: ...\n---`.

**NEVER** include `related: []` when there are no related files. **WHY:** An empty list is noise; the field should be absent. **BAD:** `related: []` **GOOD:** Omit the `related` key entirely.

**NEVER** put findings or plans under `.context/audits/`. **WHY:** `.context/audits/` is owned by `skill-auditor --store`; writing there by hand conflicts with the tool's schema. **GOOD:** Use
`.context/plans/`, `.context/findings/`, or `.context/analysis/` only.

**NEVER** report deferred or out-of-scope work only in the chat response. **WHY:** Chat context does not persist across sessions or after `/clear`/compaction; an unfiled follow-up is lost the moment
the session ends, and the next session has no way to pick it up. **BAD:** "Note: the `tooling` tag needs a facet assignment" mentioned in a reply, nothing written to disk. **GOOD:** File it as
`.context/follow-ups/YYYY-MM-DD-<topic>.md` with `status: active` in the same turn it's identified, then mention it in the response.

## References

| Topic                                                   | Reference                                                      | When to Use                                                       |
| ------------------------------------------------------- | -------------------------------------------------------------- | ----------------------------------------------------------------- |
| Four file types, field conventions, and common mistakes | [Context File Types](references/context-file-types.md)         | Choosing the correct type or debugging file placement             |
| Required and optional frontmatter fields with examples  | [YAML Frontmatter Guide](references/yaml-frontmatter-guide.md) | Setting up frontmatter for a new file or fixing validation errors |
