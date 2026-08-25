---
title: "ADR-015: Do not adopt the leontoeides/google_maps crate"
status: accepted
date: 2026-08-24
context:
  - path: ".context/findings/2026-08-24-google-maps-crate-evaluation.md"
---

**Status:** Accepted
**Date:** 2026-08-24

## Context

`svmm` depends on two Google APIs: the Directions API and the Street View Static API (metadata and
image endpoints), both built through a shared, tested layer (`src/net.rs`'s `with_retry` and
`cached_fetch`) and both using domain-specific error types tailored to this CLI. The
`leontoeides/google_maps` crate was evaluated as a possible replacement for the hand-rolled
Directions integration.

## Decision

Do not adopt `google_maps`. The crate covers Directions but has no support for the Street View
Static API, which is one of `svmm`'s two core dependencies — adopting it would leave `streetview.rs`
exactly as hand-rolled as today, running two different integration styles side by side for the
project's two Google dependencies. The crate also owns its own HTTP client and retry behavior,
which doesn't integrate cleanly with `net.rs`'s shared cache/retry layer, and its error types would
need translating back into `directions.rs`'s existing domain-specific variants for little benefit.

## Consequences

**Easier:** No new dependency, no split retry/cache behavior across the two Google integrations,
no error-type translation layer to maintain.

**Harder:** `directions.rs`'s hand-rolled polyline decoding and route/leg parsing stay
hand-maintained rather than benefiting from the crate's typed parsing. Revisit narrowly (not as a
wholesale replacement) only if a future investigation specifically finds the polyline decode path
error-prone or under-tested — that one piece was not evaluated in depth in the original finding.
