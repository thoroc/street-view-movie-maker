# Grading Effort, Value, and Themes

Every plan this skill creates carries three required frontmatter fields beyond
the standard `title`/`type`/`status`/`date`. This reference is the full rubric
for all three -- load it before drafting frontmatter, not after validation
fails.

## Effort

A T-shirt-sized total estimate (`S`/`M`/`L`), matching the skill-auditor
remediation-plan convention. It flows straight into `.context/index.yaml` so a
reader can triage plans by effort without opening each file.

- `S` -- a single session, no cross-file coordination.
- `M` -- a few sessions, touches 2-4 files or one subsystem.
- `L` -- multi-session, touches several subsystems or requires sequencing.
- `TBD` -- ONLY when sizing is genuinely blocked on something listed in Open
  Questions. AVOID guessing a number just to satisfy frontmatter validation --
  a fake `S` is worse than an honest `TBD` with a stated blocker, because it
  misleads whoever triages by effort next.

## Value

A `HIGH`/`MEDIUM`/`LOW` benefit-of-action grade, distinct from effort (cost)
and severity (risk-of-inaction). Grade against the value rubric referenced
below (leverage, consumers unblocked, reversibility) rather than guessing. It
flows into `.context/index.yaml`, where the "what's next" read protocol sorts
by value descending, then effort ascending.

Where to find the rubric: the project's `.claude/instructions/` directory
holds a `value-rubric.md` file defining the leverage/consumers/reversibility
criteria in full. Read it before grading a plan you are unsure about -- do not
guess a value grade from the plan's title alone.

## Themes

An ordered list of one or more areas from the controlled vocabulary
(`COLLECTION` / `DIGEST` / `DELIVERY` / `INFRA` / `GOVERNANCE` / `TOOLING`).
The subject axis (what area the plan touches), orthogonal to value and effort.

Write it **primary-first** -- `themes[0]` answers "what is this mainly about?"
and is the tie-breaker below value then effort.

Where to find the vocabulary: the project's `.claude/instructions/` directory
holds a `theme-vocabulary.md` file with the full, closed list plus a
split-on-evidence threshold for when a new theme is warranted. PREFER an
existing theme UNLESS the plan genuinely doesn't fit any of the six -- do not
invent a seventh without checking that file first.

## Worked example

```yaml
---
title: "Plan: Add SQS DLQ redrive to the retry sweep"
type: PLAN
status: DRAFT
date: "2026-08-01"
effort: M
value: HIGH
themes:
  - DELIVERY
  - INFRA
---
```

`DELIVERY` is primary because the redrive changes what happens to a failed
delivery; `INFRA` is secondary because it also touches the SQS/Terraform
layer. `value: HIGH` because it unblocks recovering deliveries that currently
require a manual S3 replay. `effort: M` because it touches the retry sweep,
the DLQ Terraform resource, and the runbook -- three files, one subsystem.
