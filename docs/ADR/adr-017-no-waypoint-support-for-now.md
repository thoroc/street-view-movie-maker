---
title: "ADR-017: No multi-stop/waypoint route support for now"
status: accepted
date: 2026-08-25
context:
  - path: ".context/findings/2026-08-25-no-waypoint-multi-stop-route-support.md"
---

**Status:** Accepted
**Date:** 2026-08-25

## Context

`svmm` only accepts a single origin and destination — there is no way to route through an
intermediate stop, even though Google's Directions API natively supports a `waypoints` request
parameter. The gap surfaced while comparing `svmm` against an unrelated tool
(`osingla/RouteView`) that supports an optional intermediate waypoint. No user has asked for this;
it's a feature idea from tool comparison, not a request in flight.

## Decision

Do not add multi-stop/waypoint routing support at this time. `svmm` stays strictly point-A-to-
point-B. If pursued later, it needs its own feasibility work first on: the CLI flag shape
(repeatable `--waypoint` vs. a delimited value vs. a file of stops), whether waypoints are
optimized/reordered by the Directions API or taken in the given order (this project's existing
preference for predictable, reproducible routes suggests defaulting to "as given"), how route
fingerprinting/resume accounts for an added/removed/reordered waypoint, and interaction with the
existing avoid-options feature.

## Consequences

**Easier:** No added CLI surface, no route-fingerprint redesign, no interaction risk with the
existing avoid-options feature — all deferred until there's real demand.

**Harder:** A user who wants multi-stop routing today has no path to it. Revisit only if a real
user request surfaces; at that point this ADR's context finding is the starting point for a
`plan-create` pass, not a fresh investigation.
