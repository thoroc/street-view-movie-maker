---
title: "ADR-008: Inset map overlay — local compositing, single Static Maps fetch, on-by-default"
status: proposed
date: 2026-08-23
context:
  - path: ".context/findings/2026-08-23-inset-map-with-route-in-the-video-corner-feasibility.md"
  - path: ".context/plans/2026-08-23-add-inset-map-with-route-to-the-output-video-corner.md"
---

**Status:** Proposed
**Date:** 2026-08-23

## Context

The output video has no in-frame indication of where the car is along the route. A small inset map
with a moving position marker would give the viewer that context. Two implementation strategies
were evaluated: (A) an ffmpeg `filter_complex` overlay compositing a second image input at encode
time, or (B) burning the map inset into each Street View frame locally (via the `image` crate)
before the existing single-input ffmpeg encode. A moving marker rendered by calling the Static Maps
API once per frame would multiply an already-large per-route image count by a second billed API.

## Decision

- **Composite locally (Strategy B)**, not via ffmpeg `filter_complex` — keeps ffmpeg's args
  unchanged, avoids filter-string construction/escaping, and matches this codebase's existing
  preference for logic in Rust over shell-clever ffmpeg pipelines.
- **Fetch the whole-route static map once per run** (a single Static Maps API call), and draw the
  moving position marker locally onto that cached bitmap per frame — not one Static Maps call per
  frame. Keeps the added cost to one extra API call per run, negligible next to Street View download
  cost.
- **Moving marker is required, not optional** — the feature's whole point is a live position
  indicator; the base map image itself stays static, fetched once.
- **Default corner: bottom-right**, selectable via `--map-corner`.
- **On by default**, with `--hide-map` (default false) to opt out, rather than an opt-in
  `--show-map` flag.
- **Reuse the existing API key** (`DIRECTIONS_API_KEY`, recommended, or `STREETVIEW_API_KEY`) rather
  than adding a new required env var — surface a clear auth-error message if that key's GCP project
  hasn't enabled the Maps Static API, rather than silently falling back to `--hide-map` behavior.

Placing a marker correctly requires the route's bounding box turned into an explicit `center`+`zoom`
before requesting the image (Static Maps doesn't echo back an auto-fit zoom), then a Web-Mercator
lat/lon-to-pixel projection reused for marker placement — a new geo helper, since none of the
existing `geo.rs` helpers (`haversine_meters`, `initial_compass_bearing`,
`interpolate_points*`) are projection-related.

## Consequences

**Easier:** No new filter-graph complexity in `video.rs`; cost stays predictable (one extra API
call per run); users get a working feature with no new required configuration.

**Harder:** New dependencies (`image`, possibly `imageproc`) and a new Mercator-projection helper
to build and maintain. Shipping on-by-default with a reused key means an existing user whose
`DIRECTIONS_API_KEY` project hasn't enabled Maps Static hits a hard auth failure on their very next
run — flagged in the source plan as a real rollout risk, not silently mitigated.
