---
title: "ADR-012: Switch from release-plz to release-please for versioning and changelog"
status: accepted
date: 2026-08-24
context:
  - path: ".context/plans/2026-08-24-add-release-and-changelog-pipeline.md"
---

**Status:** Accepted
**Date:** 2026-08-24

## Context

ADR-011 chose `release-plz` over `release-please` for `Cargo.toml`/`CHANGELOG.md` versioning,
reasoning that it's Rust-native and needs no JSON config file for a single-package repo.
Implementing it surfaced a structural mismatch this repo's use case exposes, not just a
misconfiguration:

- `release-plz release` defaults to running `cargo publish`. `svmm` is a CLI tool with no
  crates.io presence (`publish = false` per ADR-011's own scope), so the first real run failed
  outright with a missing-`CARGO_REGISTRY_TOKEN` error, before ever creating a tag.
- `release_always` defaults to `true`: it tries to release on every push to `main`, not only
  after a release PR merges. Combined with `Cargo.toml` already declaring `0.1.0` pre-pipeline,
  this fired an immediate, unreviewed release attempt on the very first run.
- Most significantly: **`release-plz` determines "what's already released" by querying the
  crates.io registry, even with `publish = false`.** Since `svmm` is never published there, this
  check always returns "nothing released," so `release-plz` proposes re-releasing `Cargo.toml`'s
  current version forever — completely blind to git tags, including the three historical
  releases (`v0.1.0`/`v0.2.0`/`v0.3.0`) already backfilled by hand at specific milestone commits.
  A `git_only = true` config option exists specifically for registry-free projects, but adding a
  third corrective flag on top of two others, all needed to route around defaults tuned for the
  registry-publishing case, is a signal the tool's model doesn't fit rather than a one-off
  mistake.

`release-please` has no registry concept at all: it is purely git-tag and JSON-manifest driven,
using Conventional Commits since the last tagged release (tracked in
`.release-please-manifest.json`) to decide the next version. This entire class of bug cannot occur
there. ADR-011's other objection to `release-please` — its pairing with `cargo-dist` raising an
ambiguity over which tool creates the GitHub Release — doesn't apply here, since this repo already
has its own hand-authored `release.yml` (unchanged by this decision) rather than `cargo-dist`.

## Decision

- **Replace `release-plz.yml`/`release-plz.toml` with `release-please.yml`**,
  `release-please-config.json`, and `.release-please-manifest.json`, following the pattern from
  `pantheon-org/tekhne`'s `release-please.yml` (the other reference researched in ADR-011),
  adapted for a single Rust package: `release-type: "rust"`,
  `include-component-in-tag: false` (explicit, not relying on an undocumented default, given
  today's pattern of default-related surprises), manifest seeded at `"0.3.0"` to match the
  already-backfilled `v0.3.0` tag.
- **`release.yml` (the cross-platform build/publish workflow) is unchanged in shape** — still the
  sole place a GitHub Release is created, still supports `workflow_dispatch` for historical
  backfill. It does, however, get an unrelated fix landing in the same change: `jdx/mise-action`
  is replaced with `dtolnay/rust-toolchain` for the build-only Rust install, because mise's shims
  lazily reconcile the *entire* `mise.toml` toolset (pulling in `ffmpeg`, which has no Windows
  plugin and needs `nasm` on Linux) the first time a shimmed command runs, regardless of
  `install_args` scoping the initial install step.
- **Reuse the existing `RELEASE_PLZ_TOKEN` secret** rather than ask for a new PAT — the
  permissions required (Contents read/write, Pull requests read/write) are identical regardless
  of which tool opens the PR. The name is a known cosmetic mismatch, documented in the release
  process docs, not a functional issue.
- **The historical backfill (`v0.1.0`/`v0.2.0`/`v0.3.0`) is unaffected** — those are plain git
  tags and hand-created GitHub Releases, independent of which tool manages *future* versioning.

## Consequences

**Easier:** no registry-shaped defaults to route around; `release-please`'s model (git tag +
manifest, no package registry) matches this repo's actual shape (git/GitHub-Release-only, no
crates.io) with zero corrective config. `release.yml`'s build no longer depends on `mise.toml`'s
other pinned tools at all.

**Harder:** `release-please-config.json` and `.release-please-manifest.json` are two more files
to keep in sync manually if package structure ever changes (a cost ADR-011 originally avoided by
picking `release-plz`, but one now judged smaller than the registry-mismatch cost it also carried).
This is the second release-automation tool swap in one implementation session — a cost of moving
fast on a greenfield pipeline rather than fully validating both tools' edge cases up front.

## Outcome

Pending implementation — see the linked plan for status.
