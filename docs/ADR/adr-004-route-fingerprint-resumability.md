---
title: "ADR-004: Route fingerprint and on-disk itinerary persistence for resumability"
status: accepted
date: 2026-08-22
context:
  - path: .context/plans/2026-08-22-port-street-view-movie-maker-to-rust-cli.md
  - path: .context/plans/2026-08-23-add-route-avoid-options-tolls-highways-ferries.md
  - path: .context/findings/2026-08-23-adr-candidates-from-python-to-rust-port-git-history.md
---

**Status:** Accepted
**Date:** 2026-08-22

## Context

The Python original persisted itinerary state via pandas `to_pickle` with ad hoc `downloaded_1`/`downloaded_array`
tracking, with no protection against silently resuming into a *different* route under the same output name. A
review round flagged this as a gap: no route-identity check on resume.

## Decision

Persist the itinerary to `<output-dir>/itinerary.json` (serde) after each probe/download batch, alongside a
fingerprint hash of the raw `--from`/`--to` input strings (not the geocoded result, so the fingerprint stays stable
across minor geocoding drift) plus every tuning flag (hop-size, turn-threshold, picsize, fov, pitch, radius). On
startup, a matching fingerprint resumes (skipping already-probed/downloaded rows); a mismatch is a hard error naming
the conflict, requiring `--fresh` to proceed. `--fresh` always skips the check and starts clean.

## Consequences

Gains: resuming an interrupted run never silently mixes two different routes under one output name, and completed
(billed) work is never silently re-billed on a re-run. Cost: the fingerprint hash is a persistence-format
commitment — adding a new tuning field changes the hash for every existing `itinerary.json`, not just runs that use
the new field. This already happened in practice: the `--avoid-tolls`/`--avoid-highways`/`--avoid-ferries` plan's
own review confirmed that adding those flags to `TuningParams` invalidates every pre-existing itinerary file on
upgrade, forcing `--fresh` even for unrelated runs. Any future tuning flag addition inherits this same cost and
should say so explicitly rather than claim zero risk.
