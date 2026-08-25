---
title: "ADR-020: No frame-transition smoothing for uneven Street View capture density, for now"
status: accepted
date: 2026-08-25
context:
  - path: ".context/findings/2026-08-25-frame-transition-smoothing-via-path-optimization.md"
---

**Status:** Accepted
**Date:** 2026-08-25

## Context

`svmm` samples frames at a fixed hop distance and dedupes only exact-duplicate pano IDs, with no
mechanism to smooth over visually jarring — but visually distinct — consecutive frames caused by
uneven real-world Street View capture density. `pelmers/streetwarp-cli` (same domain: route to
Street View hyperlapse) solves this with a second pass: feature detection, matching, and homography
fitting between candidate frame pairs to compute a "visual-jump cost," combined with a pacing cost,
resolved via a shortest-path DP over a DAG to pick the best frame subsequence.

## Decision

Do not implement homography/path-optimization-based frame-transition smoothing at this time.
`svmm` keeps fixed-distance sampling and exact-pano-ID dedupe only. This is a real, structural gap
in output quality, but no user has reported it, and adopting the technique wholesale is substantial
new scope — either a Rust computer-vision path (feature detection + homography + RANSAC, crate
maturity unverified) or an out-of-process helper, plus a shortest-path optimization pass wired into
the existing frame pipeline. If pursued, it needs its own `plan-create` pass with a feasibility
finding first (crate maturity, performance cost of feature-matching over potentially thousands of
frames, and interaction with the existing turn-frame-insertion and inset-map/turn-sign compositing
passes).

## Consequences

**Easier:** No new computer-vision dependency, no second-pass frame-selection architecture to
maintain alongside the existing pipeline.

**Harder:** Routes through areas with uneven Street View capture density will continue to produce
visually jarring jump-cuts between geographically-close-but-visually-discontinuous frames. Revisit
if this is reported as a real user complaint, or picked up proactively as a quality investment.
