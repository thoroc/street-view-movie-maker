---
title: "ADR-019: No Google Static Maps provider swap yet for the inset map; Mapbox is the leading candidate"
status: accepted
date: 2026-08-25
context:
  - path: ".context/findings/2026-08-25-inset-map-provider-cost-alternatives-to-google-static-maps.md"
---

**Status:** Accepted
**Date:** 2026-08-25

## Context

The inset-map feature's one-off Google Static Maps request already costs roughly $0.002/run —
negligible on today's observed spend. The actual driver for investigating alternatives is
*potential* cost/billing exposure (a future Google Maps Platform pricing change, and the inset
map's key currently sharing `DIRECTIONS_API_KEY`/`STREETVIEW_API_KEY`, so a leaked or abused key has
a blast radius covering all three Google products at once). Mapbox, MapTiler, Geoapify, and
LocationIQ were evaluated as static-map-image alternatives.

## Decision

No provider swap yet. Mapbox is the leading candidate if/when this is pursued: its overlay syntax
(`path-{width}+{color}-{opacity}(polyline)`) is a near-drop-in for `src/maps.rs`'s existing
Google-style encoded-polyline builder, and — unlike the OSM-derived alternatives (MapTiler,
Geoapify, likely LocationIQ) — it carries no new attribution obligation to add to the frame
compositing pass. Before scoping a plan, three things need resolving: Mapbox's exact current
Static Images API free-tier allowance and per-request cost beyond it (its pricing page renders
those figures via client-side JS, not visible to an automated fetch), whether a fully separate
Mapbox account/key is the intent (to actually achieve the blast-radius reduction motivating this),
and a ToS review confirming Mapbox's own required attribution wording/placement.

## Consequences

**Easier:** No swap work is started until the cost/blast-radius rationale and the three open items
above are actually resolved, avoiding a premature integration against unconfirmed pricing figures.

**Harder:** The shared-key blast-radius risk (a single leaked/abused key affecting Directions,
Street View, and the inset map together) remains unaddressed until this is revisited. Revisit when
either the open items above are resolved or the billing-risk driver is judged serious enough to
prioritize.
