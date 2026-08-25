---
title: "ADR-022: ADR status-drift check design — block on proposed→accepted readiness, supersedes_adr replaces context:-based supersession linking"
status: proposed
date: 2026-08-25
context:
  - path: ".context/plans/2026-08-25-detect-adr-status-drift-in-ci.md"
---

**Status:** Proposed
**Date:** 2026-08-25

## Context

ADR-008 sat at `status: proposed` for two days after the finding and plan it documents both
shipped (`status: done`) and the feature it describes was implemented and tested — nothing caught
the drift until it was found by chance and fixed manually in PR #30. A plan to add a CI/pre-commit
check for this class of drift (both `proposed`→`accepted` readiness and `accepted`→`superseded`
consistency) surfaced two decisions during a `guided-interview` and a third during `plan-review`,
once a Technical reviewer found that `.claude/skills/adr-capture/references/adr-supersession.md`
already documents a competing convention for linking a superseding ADR back to the one it replaces.

## Decision

- **The `proposed`→`accepted` readiness check blocks the commit outright**, matching the existing
  `adr_undocumented_decisions` precedent in `hk.pkl`, rather than only warning. Rejected
  alternatives: warn-only (no teeth — the exact failure mode that let ADR-008 drift unnoticed for
  two days); warn-then-escalate after repeat occurrences (needs state tracking with no precedent in
  this repo's hook scripts).
- **The field linking a superseding ADR to the one it replaces is named `supersedes_adr: adr-NNN`**,
  added as a new optional field to the ADR frontmatter schema — not `supersedes` (reads ambiguously
  as a verb phrase) and not a schema-free internal-consistency-only check (can't detect a forgotten
  update on the old ADR when a new one is created, since nothing links forward to find it).
- **`supersedes_adr` replaces the `context:`-based supersession-linking convention** that
  `adr-supersession.md` already documented (its Process step 5 and Key Rules table said to
  reference the superseded ADR via the new ADR's `context:` field). Investigation found zero ADRs
  in this repo actually use that convention — the one real supersession (ADR-011 → ADR-012,
  2026-08-24) does not follow it. Retiring it costs nothing (no migration needed) and avoids two
  competing mechanisms describing the same relationship; `adr-supersession.md` will be updated to
  describe `supersedes_adr` when the check itself is implemented.

## Consequences

- A `proposed` ADR whose linked findings/plans are all `done` will fail pre-commit until a human
  flips its status via a normal branch/PR — consistent with this repo's rule that a status change is
  a governance decision, never an automatic write.
- `adr-supersession.md`'s worked example and Key Rules table need updating alongside the check's
  implementation so the documented process and the enforced one agree.
- This check's enforcement boundary is the local `hk` pre-commit hook only: `.github/workflows/
  ci.yml` excludes `docs/**`/`**/*.md` from its triggers, so an ADR-only change never runs CI at
  all. A GitHub web-UI edit, a squash-merge without `hk` installed, or `git commit --no-verify` all
  bypass this check — the same exposure `adr_undocumented_decisions`/`context_filenames` already
  carry. Closing that gap would require removing the `docs/**` CI exclusion, a separate, larger
  change not undertaken here.
