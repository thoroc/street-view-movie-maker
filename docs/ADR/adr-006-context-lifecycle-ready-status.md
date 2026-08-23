---
title: "ADR-006: Split active into ready and active in the .context lifecycle"
status: proposed
date: 2026-08-23
context:
  - path: ".context/findings/2026-08-23-plan-status-vocabulary-conflates-ready-and-active.md"
  - path: ".context/plans/2026-08-23-add-ready-status-to-context-lifecycle.md"
  - path: ".context/findings/2026-08-23-undocumented-decisions-check-has-false-negative-on-index-yaml-mention.md"
  - path: ".context/follow-ups/2026-08-23-undocumented-decisions-check-not-wired-into-hk-pkl.md"
  - path: ".context/follow-ups/2026-08-23-context-frontmatter-enum-values-out-of-sync-with-docs.md"
---

**Status:** Proposed
**Date:** 2026-08-23

## Context

The `.context/` frontmatter `status` field's enum (`draft, active, done, superseded`) asks `active`
to mean two different things at once: "reviewed and approved, queued to start" and "someone is
implementing this right now." `planning-flow.md` promotes a plan from `draft` to `active` purely on
user approval, with no later transition marking when work actually begins. This surfaced concretely,
not hypothetically: `.context/index.yaml` currently lists two plans both at `status: active`, where
only one was believed to be under active implementation — and a repo-state check during review found
neither actually had an associated worktree or branch, meaning the ambiguity was real in practice,
not just in theory.

This is a shared-contract change: the field is read by `value-rubric.md`'s "what's next" sort
protocol, `theme-vocabulary.md`'s own copy of that protocol, `planning-flow.md`'s promotion
instructions, and every skill (`context-file`, `plan-create`, `plan-review`, `context-index`) that
creates, reads, or validates `.context/` frontmatter.

## Decision

Split `active` into two states so the lifecycle becomes `draft → ready → active → done`
(`superseded` remains an escape hatch from any state):

- `ready` — reviewed, approved, queued; not yet started.
- `active` — implementation is actually underway.

The `ready → active` transition is made **observable rather than self-declared**: a plan is `active`
while a worktree or branch referencing it exists (created via `EnterWorktree`, per
`ways-of-working.md`'s Branch workflow), and reverts to `ready` or advances to `done` when that
worktree/branch is closed out. This was a deliberate choice over two alternatives considered and
rejected: pure self-declaration (too easy to leave stale, no correction mechanism) and tying it to a
session-log/observation entry (not part of the `.context/` system, not durable).

The read protocol's tier order becomes `active > ready > draft` (each still sorted by `value` then
`effort` internally), so in-progress work surfaces above queued work, which surfaces above unreviewed
drafts.

Scope is deliberately narrow: only `type: plan` gets this split. `finding` and `follow-up` keep
`active` meaning "still outstanding/unresolved" — they have no implementation-in-progress concept
the way plans do, so widening the split to them was considered and explicitly rejected.

Explicitly out of scope, filed separately rather than folded in: the pre-existing gap where
`value-rubric.md` documents a `DEFERRED` status and `KNOWN_ISSUE` type that the schema's enums have
never actually included (see
`.context/follow-ups/2026-08-23-context-frontmatter-enum-values-out-of-sync-with-docs.md`) — a
different defect (a missing documented value) than this ADR's problem (one value doing two jobs).

## Consequences

**Easier:**

- A reader (human or agent) can tell "queued" from "in progress" at a glance, without checking
  session memory or asking.
- The "what's next" sort can surface genuinely active work above merely-queued work.
- The transition is checkable against repo state (worktree/branch existence) instead of resting on
  memory or an unenforced convention.

**Harder / new constraints:**

- The `active` signal only works if `ways-of-working.md`'s worktree-first rule is actually followed.
  Work done by editing files directly in a shared checkout (as happened with one of the two plans
  that motivated this ADR) won't be detected as `active` — this is a pre-existing gap in following
  that rule, not one this decision introduces, but this decision now makes it visible as a false
  "ready" reading rather than a silent one.
- `.context/` remains local, gitignored, unsynced state (per this repo's own convention — it is
  explicitly not an audit trail). This decision does not add any cross-machine or cross-worktree
  sync mechanism for re-triaged status values; a worktree that needs current `.context/` state still
  copies it in on entry, same as before this change.
- Every skill and instruction file that names status values by name (`context-file`, `plan-create`,
  `plan-review`, `context-index`, `value-rubric.md`, `theme-vocabulary.md`, `planning-flow.md`) needs
  a coordinated documentation sweep; a partial sweep leaves stale `ACTIVE`-only language that
  contradicts the new five-value lifecycle.

**Scope amendment (2026-08-23, same day, user directive):** three defects surfaced while reviewing
this decision — missing `deferred`/`known-issue` schema enum values, a false-negative bug in
`check-undocumented-decisions.ts`, and that script never actually being wired into `hk.pkl` despite
being documented as a hard gate — were folded directly into this plan's scope rather than filed and
deferred ("fix forward"). Wiring the gate on required backfilling ADRs for 5 pre-existing
undocumented decisions it surfaced (see ADR-007 through ADR-010) so the newly-blocking check doesn't
immediately fail on unrelated debt.
