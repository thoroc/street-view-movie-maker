---
title: "ADR-016: Do not build features on Google's unofficial Street View depth-map endpoint"
status: accepted
date: 2026-08-25
context:
  - path: ".context/findings/2026-08-25-streetview-explorer-depth-data-not-usable.md"
---

**Status:** Accepted
**Date:** 2026-08-25

## Context

`PaulWagener/Streetview-Explorer` demonstrates walking around inside a Street View panorama in 3D
by downloading Google's per-panorama depth-map data and projecting the photo onto its depth mesh.
The technique is genuinely interesting in the abstract — depth data for two consecutive route
frames could in principle enable smoother 3D transitions between viewpoints, adjacent to the
frame-jump problem this project separately tracks. But the depth data comes from an undocumented,
reverse-engineered Google endpoint, never part of the official, licensed Street View Static API
`svmm`'s billed Maps Platform key operates under.

## Decision

`svmm` will not build features on Google's unofficial/undocumented Street View depth-map endpoint,
now or in the future, regardless of whether it "still works." Only the officially licensed Street
View Static API is used. An unofficial endpoint carries no supported contract, SLA, or version
guarantee, and — per this project's regulatory posture as a PLG project — relying on undocumented
third-party API access for a shipped feature is a compliance risk to be routed through
Legal/Compliance, not adopted unilaterally.

## Consequences

**Easier:** No feature is ever built on infrastructure that could disappear or change without
notice, and no compliance review is ever needed for this specific technique since it's ruled out
categorically rather than case-by-case.

**Harder:** The frame-transition-smoothing problem this technique could have addressed stays
unsolved by this route; any future fix needs a different technique (e.g. the homography/feature-
matching approach already tracked separately) that doesn't depend on unlicensed API access.
