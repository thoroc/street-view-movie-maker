---
title: "ADR-018: No self-hosted OSRM routing backend for now"
status: accepted
date: 2026-08-25
context:
  - path: ".context/findings/2026-08-25-osrm-self-hosted-alternative-to-google-directions.md"
  - path: ".context/findings/2026-08-25-inset-map-provider-cost-alternatives-to-google-static-maps.md"
---

**Status:** Accepted
**Date:** 2026-08-25

## Context

Unlike Street View, `svmm`'s Google Directions API dependency has a real substitute: self-hosted
OSRM (Open Source Routing Machine) against a regional OpenStreetMap extract, with no API key and no
billed request. This was found while evaluating an unrelated tool (`billygl/timelapse`), which uses
exactly this approach. It would remove billing/pricing-change exposure entirely rather than moving
it to another commercial provider, at the cost of a real self-hosting burden (a Docker container
plus a regional OSM extract matched to wherever the route runs) and no live-traffic awareness.

## Decision

Do not add a self-hosted OSRM routing backend now. The Directions API stays the sole routing
source. This would be a significant architectural change (an optional self-hosted backend, not a
drop-in API swap), and no user has asked for offline/self-hosted routing specifically. If the
potential Google Maps Platform billing/pricing exposure identified in the Static Maps
cost-alternatives finding is judged serious enough to also cover Directions, OSRM is worth a
feasibility pass as its own plan, scoped as an **optional** `--routing-backend osrm|google` choice
rather than a hard replacement — the self-hosting and coverage tradeoffs aren't acceptable defaults
for every user.

## Consequences

**Easier:** No new infrastructure dependency (Docker, regional OSM extracts) for users who don't
need it; the existing avoid-options feature (tolls/highways/ferries) doesn't need re-mapping onto
OSRM's different profile/exclude syntax yet.

**Harder:** Every `svmm` user remains fully dependent on Google's Directions API pricing and
availability, with no self-hosted fallback. Revisit only if the billing-risk driver is judged
serious enough, or a user specifically requests offline/self-hosted routing.
