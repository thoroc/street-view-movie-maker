---
title: "ADR-001: Rewrite street-view-movie-maker from Python 2 to a Rust CLI"
status: accepted
date: 2026-08-22
context:
  - path: .context/plans/2026-08-22-port-street-view-movie-maker-to-rust-cli.md
  - path: .context/findings/2026-08-23-adr-candidates-from-python-to-rust-port-git-history.md
---

**Status:** Accepted
**Date:** 2026-08-22

## Context

The original tool (`utils.py` + `street_crawl.py`) was a Python 2 script (`raw_input()`, pandas `to_pickle()`) with
no packaging, no tests, and a bespoke one-off script (`hollerado_project.py`) mixed into the same tree. It needed a
real rewrite rather than incremental patching: no persistence/resumability, no cost gate before billed downloads,
and no test coverage against the original's known-good outputs.

A TypeScript/Bun local server with a web interface was considered as an alternative shape. It was rejected: a
job/progress-state design (SSE or polling), API-key-exposure risk in a server context, and file storage/caching
concerns are all unwarranted for a tool that's fundamentally a one-shot CLI script — they would have added an
architecture layer the actual use case doesn't need.

## Decision

Rewrite the reusable pipeline (route → Street View frames → video) as a Rust CLI (`svmm`), leaving
`hollerado_project.py` unported as a reference-only artifact for a specific past project. Port in six phases
(CLI scaffold, geo math, Directions API, Street View probing/download, lineup/dedupe, video encoding), each behind
its own test suite, verified line-for-line against the Python original's known-good outputs where a direct
comparison was possible (geo math, dedupe logic).

## Consequences

Gains: static typing, a real test suite (79 passing tests as of the port's completion), resumable/persisted state,
a cost gate before billed downloads, and no runtime dependency on a Python environment. Costs: the rewrite is a
one-way door — the Python implementation was subsequently deleted (`c070bf9`) rather than kept in parallel, so any
future feature work happens exclusively in Rust, and any behavior only the Python version had (that wasn't
explicitly ported, e.g. grid/mosaic capture) requires re-implementation from scratch if ever needed again.
