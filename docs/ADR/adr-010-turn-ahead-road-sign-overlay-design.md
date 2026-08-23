---
title: "ADR-010: Turn-ahead road-sign overlay — maneuver-driven, render-time windowing"
status: proposed
date: 2026-08-23
context:
  - path: ".context/findings/2026-08-23-turn-ahead-road-sign-overlay-feasibility.md"
  - path: ".context/plans/2026-08-23-add-turn-ahead-road-sign-overlay.md"
---

**Status:** Proposed
**Date:** 2026-08-23

## Context

The feature shows a road-sign-style overlay 2-3 seconds before a real navigation turn, sourced from
Directions API maneuver data — distinct from the existing geometric heading-delta smoothing in
`itinerary::insert_turn_frames`, which stays unrelated and unchanged. A requirement surfaced that
"straight on" must also get a sign whenever an intersection could otherwise be misread as needing a
turn, which needed a concrete rule rather than a bespoke ambiguity heuristic.

## Decision

- **"Straight on" signage maps directly onto Google's own `maneuver` field semantics**: show a sign
  for every step with a non-empty `maneuver` field (including `maneuver == "straight"`, which Google
  already populates specifically for ambiguous intersections/forks); show no sign for steps with no
  `maneuver` field at all (plain continuations Google itself doesn't consider instruction-worthy).
  `TurnDirection` gets an explicit `StraightOn` variant driven by `maneuver == "straight"`.
- **Match maneuvers against the post-dedupe point list** (after `dedupe_by_pano_id`, before
  `insert_turn_frames`), not the pre-dedupe list — matching pre-dedupe risks the matched point being
  filtered out, silently losing the sign. Skip (with a warning) any maneuver whose nearest match
  exceeds a small multiple of `--hop-size`, rather than tagging a visibly wrong point.
- **Timing computed at render/compositing time**, not baked into `PointRecord` at itinerary-build
  time: for each frame, scan forward through the maneuver list for the nearest one within the lead
  window, clamped to frame 0. This (not a fixed per-point window) is what lets two maneuvers closer
  together than the lead window each still get their own sign as their respective frame approaches.
- **Matched maneuvers persist as `ItineraryFile.maneuvers: Vec<Maneuver>`**, not as a per-`PointRecord`
  field — survives `insert_turn_frames`'s wholesale clone automatically and persists across a resume.
- **`--turn-sign-lead-seconds`/`--show-turn-signs` stay out of `route_fingerprint`** — since matching
  happens once and windowing is computed at render time, changing the lead time on a resumed run
  just changes what the same persisted maneuver data renders as, with no stale-state risk (same
  principle already applied to `--map-size`/`--map-corner` in the inset-map decision, ADR-008).

## Consequences

**Easier:** Reuses Google's own maneuver semantics instead of a bespoke ambiguity heuristic; render-time
windowing handles close-together maneuvers correctly without extra state; resume behavior needs no
special-casing for the new flags.

**Harder:** New font-rendering dependency (e.g. `ab_glyph`) and a bundled font file to manage
(license permitting redistribution). Shares the inset-map plan's per-frame compositing pass, so it
must land after that plan (ADR-008) rather than independently.
