---
title: "ADR-021: Repo-scouting log stays gitignored under .context/, as JSONL"
status: accepted
date: 2026-08-25
context:
  - path: ".context/plans/2026-08-25-add-repo-scouting-log-skill.md"
---

**Status:** Accepted
**Date:** 2026-08-25

## Context

A new skill logs the outcome of investigating external GitHub repos for techniques `svmm` could
learn from or port, so a future session doesn't re-investigate the same repo from scratch. Two
questions needed resolving during plan review: where the log lives, and what format it uses. The
log's entire justification — that future sessions can find past verdicts — is in tension with
`.context/` being documented project-wide as gitignored, per-worktree scratch, which independent
Strategic and Risk plan reviewers both flagged as the same problem the log exists to solve.

## Decision

The log stays at `.context/repo-scouting/log.jsonl`, gitignored like every other `.context/`
artifact, rather than moving to a committed `docs/` file. This is explicitly a research log, not a
decision registry (an investigation that produces a real decision still gets its own finding or
plan, and an ADR if warranted, as this one now points at seven such findings via
ADR-014 through ADR-020) — moving only this one research artifact to `docs/` while every other
finding stays gitignored scratch would be an inconsistent, one-off exception to a boundary this
project draws intentionally. The accepted tradeoff: a solo developer working across worktrees on
one machine manually syncs `.context/repo-scouting/log.jsonl` back to the main checkout the same
way any other `.context/` file is synced, rather than getting automatic cross-worktree/cross-clone
persistence.

Format is JSONL — one JSON object per line — chosen after correcting the plan's original,
inaccurate rationale (that JSONL avoids "whole-file-rewrite races across worktrees"; worktree
isolation happens at the directory level regardless of file format). The real reasons: appends
within one copy never require a whole-file read-modify-write, and because `.jsonl` is not a `.md`
file, it falls outside `check-undocumented-decisions.sh`'s `.context/**/*.md` scan entirely — which
matters because a `reasoning` free-text field alongside a `PORT_CANDIDATE` verdict would otherwise
look exactly like the decision-language that scan exists to catch.

## Consequences

**Easier:** No governance-model exception carved out for one artifact; entries are cheap to append
programmatically; the log format sidesteps false-positive risk against the decision-detection hook
by construction.

**Harder:** The log has no automatic enforcement or cross-worktree sync — a session that forgets to
sync it back to the main checkout, or whose worktree is removed before syncing, loses those
entries permanently. Revisit if this project ever gains multiple contributors or clones (not just
worktrees on one machine), at which point the manual-sync model stops working entirely.
