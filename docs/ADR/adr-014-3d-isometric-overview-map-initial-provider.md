---
title: "ADR-014: Static map API (Mapbox) as the initial path for the 3D/isometric overview-map feature"
status: accepted
date: 2026-08-24
context:
  - path: ".context/findings/2026-08-24-3d-isometric-overview-map.md"
---

**Status:** Accepted
**Date:** 2026-08-24

## Context

An optional 3rd-person/isometric-style overview map (a tilted Google-Maps-3D-style view) was
evaluated as a possible addition alongside `svmm`'s street-level footage. Three approaches were
considered: (1) headless-browser rendering of MapLibre GL JS, (2) a static-image HTTP API with
pitch/bearing parameters, and (3) native Rust 3D rendering (`wgpu`) over vector tiles. Option 1
reintroduces an external runtime dependency (Chromium) the project just moved away from for video
encoding; Option 3 is a substantial sub-project (own tile decoding, extrusion, camera logic), not a
quick add.

## Decision

Start with Option 2 — a static map image API — to validate the feature quickly, with Mapbox as the
reference/primary provider (best-documented pitch/bearing and 3D-building-extrusion support of the
options surveyed; Geoapify is a known-but-unevaluated alternative; Google Static Maps and MapTiler
were ruled out or left unverified for pitch/bearing support). Option 3 (native Rust rendering)
remains the long-term, architecture-aligned option if Option 2 proves the feature out.

Supporting implementation decisions made alongside this: the capability is a runtime flag, not a
compile-time Cargo feature (always compiled in, inert unless enabled); no API key is baked into the
release binary (users supply their own, consistent with the project's ffmpeg/AV1-removal
philosophy of no external dependency baked into the shipped binary); and the feature fails clearly
with an error message if enabled without a valid key, rather than silently skipping.

## Consequences

**Easier:** A working overview-map prototype can ship without first building a Rust-native 3D
rendering stack; the static-image approach fits directly into the existing per-point frame
generation loop with a simple HTTP call per frame.

**Harder:** Ties the initial implementation to a specific third-party provider (Mapbox) and its
API/pricing terms; flag naming (`--mapbox-overview-map` vs. a provider-agnostic
`--overview-map --provider mapbox`) needs deciding before implementation to avoid a breaking rename
if a second provider is added later. Revisiting Option 3 later means the static-image
implementation may need to coexist with, or be replaced by, a fully different rendering approach.
