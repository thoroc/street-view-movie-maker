---
title: "ADR-011: release-plz plus a hand-authored release workflow for versioning, changelog, and cross-platform releases"
status: superseded
superseded_by: "adr-012"
date: 2026-08-24
context:
  - path: ".context/findings/2026-08-24-release-and-changelog-automation.md"
  - path: ".context/plans/2026-08-24-add-release-and-changelog-pipeline.md"
---

**Status:** Proposed
**Date:** 2026-08-24

## Context

`svmm` had no release workflow and no `CHANGELOG.md`. Two reference implementations were
researched: `pantheon-org/tekhne` (`release-please` + tool-generated `cargo-dist`) and `jdx/hk`
(`release-plz` + a hand-authored tag-triggered build/release workflow). `release-please` needs a
JSON config pair even for a single package and, combined with `cargo-dist`, raises an ambiguity
over which tool creates the GitHub Release (both can, by default). `release-plz` versions
`Cargo.toml`/`CHANGELOG.md` natively from Conventional Commits with no config file needed for a
single-package repo, and hk's shape has exactly one place (`create-release`) that ever calls
`gh release create`, avoiding the ambiguity structurally.

Two further live-checked facts shaped the decision: `main`'s GitHub ruleset (id `21248551`)
governs the branch only, not tags, so a tag-triggered release workflow isn't gated by it; and
GitHub does not run `pull_request`-triggered workflows (like `ci.yml`) on a PR opened by the
default `GITHUB_TOKEN` bot identity, so `ci.yml` would never run on the `release-plz` PR without
a real user's token.

## Decision

- **Use `release-plz`, not `release-please`+`cargo-dist`**, for `Cargo.toml`/`CHANGELOG.md`
  versioning from Conventional Commits, running on push to `main`.
- **Hand-author `release.yml`** (modeled on `jdx/hk`, stripped of jdx-specific private-runner and
  `communique` steps) rather than generate it via `cargo-dist`. It builds
  `x86_64`/`aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc` on
  standard GitHub-hosted runners, triggered by a `v[0-9]+.*` tag push or a `workflow_dispatch`
  naming an existing tag to build. `create-release` is the sole place a GitHub Release is ever
  created; it detects a pre-existing release (`gh release view`) and uploads to it
  (`gh release upload --clobber`) rather than duplicating, which is also what makes historical
  backfill possible (see below).
- **`release-plz.yml` uses a fine-grained PAT** (`secrets.RELEASE_PLZ_TOKEN`) with a
  `|| secrets.GITHUB_TOKEN` fallback, so the release PR is authored as a real user and `ci.yml`
  runs on it before merge — degrading gracefully (no CI on that PR, not a hard failure) if the
  secret is ever removed.
- **Backfill three historical releases** (`v0.1.0`/`v0.2.0`/`v0.3.0` at pre-existing milestone
  commits) via `workflow_dispatch` from `main` with a `version` input, rather than pushing tags
  directly at those commits — a tag pointing at a commit that predates `release.yml`'s existence
  has no copy of that workflow file in its tree, so a plain tag push would silently trigger
  nothing.
- **Release PR merge stays manual**, like every other PR in this repo — no auto-merge automation,
  regardless of the ruleset's `required_approving_review_count: 0`.
- **Scope**: no `crates.io` publishing, no Homebrew/Scoop taps, no macOS code-signing/notarization
  in this pass — unsigned binaries are an accepted, documented limitation.

## Consequences

**Easier:** version bumps and changelog entries stop being manual; a maintainer's only action is
reviewing and merging one PR per release. Cross-platform binaries appear on the Releases page
without local cross-compilation. The `create-release` existing-release check makes the workflow
reusable for both the ongoing `release-plz` flow and one-off historical backfills without two
separate implementations.

**Harder:** the pipeline now depends on a repo secret (`RELEASE_PLZ_TOKEN`) that a future
maintainer must know to rotate before it expires — documented in the release-process docs, not
just in this ADR. `main`'s ruleset has no required-status-check rule and 0 required reviews, so
the `release-plz` PR (like any PR here) can technically merge without a passing `ci.yml` run even
with the PAT in place if a reviewer doesn't wait for it — a pre-existing gap this decision does
not fix, tracked separately. Unsigned binaries will show Gatekeeper/SmartScreen warnings to end
users.

## Outcome

Pending implementation — see the linked plan for status.
