---
title: "ADR-023: Off-route Street View frame filtering: threshold source, retroactive-run scope, and shipping despite unproven benefit"
status: accepted
date: 2026-08-26
context:
  - path: ".context/findings/2026-08-25-off-route-misdirected-streetview-frames.md"
  - path: ".context/plans/2026-08-25-filter-off-route-streetview-frames.md"
---

**Status:** Accepted
**Date:** 2026-08-26

## Context

A real run turned up three downloaded frames whose Street View match was wrong in some way (facing
backward, facing a wall, on a different street). The investigating finding traced the gap to two
things: the metadata response's matched-panorama `location` was being discarded, so nothing could
detect a match far from the queried point; and there was no way to drop a specific known-bad frame
before compositing without re-downloading the whole run. The implementation plan needed a threshold
formula for the new distance check, a decision on how already-downloaded runs should be treated, and
(after validating the premise) a decision on whether to ship the distance filter at all once it
turned out not to explain any of the three motivating frames.

None of these three choices is architecture-level under this project's own ADR bar (a data store, an
integration boundary, a cross-cutting pattern, a security posture) — they're implementation-level.
This ADR exists anyway because this repo's pre-commit `check-undocumented-decisions.sh` hook flags
any `.context/` file with a `## Decisions`/`## Recommended Action` heading that isn't referenced by
an ADR, regardless of the decision's actual weight, and treats it as a hard commit-blocking failure.
This is the proportionate record the hook requires, not a claim that these choices are individually
consequential enough to warrant one on their own merits.

## Decision

1. **Distance threshold source**: derive the pano-match rejection threshold from `hop_size` using
   the same relative formula (`hop_size_m * a multiplier`) the existing turn-sign proximity filter
   (`filter_maneuvers_by_proximity`) already uses, rather than a fixed constant or a new CLI flag —
   for consistency with the one existing precedent for this exact kind of check in the codebase.
2. **Retroactive handling of already-downloaded runs**: leave pre-change `itinerary.json` records
   (no `pano_location`) untouched by the new automatic distance check rather than backfill and
   re-apply it. The backfill alternative was rejected outright, not merely deferred: `itinerary.json`
   is rewritten by `dedupe_by_pano_id`/`insert_turn_frames` on every resume, and `download_frames`
   trusts an already-set `downloaded` flag without checking the file on disk still matches — applying
   a new filter retroactively could shift list positions while already-downloaded files stay under
   their old `frameN.jpg` names, silently mismatching frames to records. Cleanup of an existing run
   goes solely through the manual `--exclude-frames` mechanism instead.
3. **Ship the automatic distance filter despite no evidence it helps on the route that motivated
   it**: re-probing the three known-bad frames found all three within 1.8–4.5m of the queried point,
   and a 497-sample survey of the full 12,406-record route found no match beyond 22.0m — no genuine
   location mismatch anywhere on this real route. The filter ships anyway as a defensive guard for
   other routes/regions where Street View coverage may be sparser or less accurate; the three
   motivating frames are addressed only by the manual exclusion mechanism.

## Consequences

**Easier:** The threshold formula reuses an existing, tested pattern instead of introducing a second
one. A resumed run against a pre-change `itinerary.json` keeps working without a migration step.
Cleanup of a specific bad frame in any run — old or new — has one mechanism (`--exclude-frames`)
instead of two divergent ones.

**Harder:** The automatic filter's real-world value is unproven — it may never fire on typical routes,
making it dead code in practice until a route with genuinely poor coverage exercises it. If that
never happens, a future cleanup pass may reasonably ask whether to keep it. The manual exclusion
mechanism is the only way to handle a wrong-facing-but-in-radius panorama (e.g. the backward-facing
case); there is no automatic detection for it, and none is planned — see the plan's Out of Scope
section.
