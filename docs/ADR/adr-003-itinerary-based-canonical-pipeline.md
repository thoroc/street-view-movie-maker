---
title: "ADR-003: Itinerary-based probe-dedupe-confirm-download as the canonical pipeline"
status: accepted
date: 2026-08-22
context:
  - path: .context/plans/2026-08-22-port-street-view-movie-maker-to-rust-cli.md
  - path: .context/findings/2026-08-23-adr-candidates-from-python-to-rust-port-git-history.md
---

**Status:** Accepted
**Date:** 2026-08-22

## Context

The Python original had two download paths: a simpler point-walk (`download_images_for_path`) and a more complex
itinerary/dataframe-based flow (`create_itinerary_df` → `probe_itinerary_items` → `process_pointlist` →
`download_pics_from_list`) with cost control (dedupe before download) and persistence. Porting both would have left
the canonical path ambiguous — a gap an early review round flagged explicitly.

## Decision

Port only the itinerary-based flow as the single canonical pipeline, hard-ordered regardless of internal
concurrency: resolve the route (billed, once) → probe Street View metadata concurrently (free) → dedupe by
`pano_id` and insert turn frames → print resolved locations, image count, and an estimated cost, then gate on
confirmation (or `--dry-run` exit) → download only deduped, `status == "OK"` images with bounded concurrency →
lineup/rename → ffmpeg encode. The simpler point-walk and `generate_download_sequence` are explicitly out of scope,
superseded by this flow.

## Consequences

Gains: cost control is structural, not incidental — no billed *download* call can happen before the user has seen a
final count and confirmed. Every module's responsibility is pinned to a pipeline stage (`itinerary.rs` owns
dedupe/turn-frames/persistence, `streetview.rs` owns probing/download, `lineup.rs` owns post-download dedupe).
Cost: the pipeline's stage order is now load-bearing — steps 4 and 5 are a hard boundary in the code, not just
documentation, so any future feature that wants to skip or reorder confirmation must treat this as a real design
change, not a one-line edit.
