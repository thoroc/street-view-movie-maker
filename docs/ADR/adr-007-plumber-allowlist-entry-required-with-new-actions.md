---
title: "ADR-007: Every new third-party GitHub Action requires a matching Plumber allowlist entry in the same commit"
status: accepted
date: 2026-08-23
context:
  - path: ".context/findings/2026-08-23-ci-cargo-mise-caching-gap-and-plumber-action-allowlist.md"
---

**Status:** Accepted
**Date:** 2026-08-23

## Context

Plumber's `githubActionMustComeFromAuthorizedSources` control requires every non-`actions/*`/
`github/*` action referenced from a workflow to be listed under `.plumber.yaml`'s
`trustedGithubActions` (deliberate defense-in-depth against supply-chain attacks like the
tj-actions/changed-files compromise, CVE-2025-30066). This repo has hit the same surprise twice
now: once adding `jdx/mise-action`, and again adding `Swatinem/rust-cache` for CI caching — both
times the missing allowlist entry only surfaced as a Plumber-blocked PR after the fact, rather than
being added proactively in the same change.

## Decision

Every new third-party GitHub Action added to any workflow in this repo gets its
`.plumber.yaml` `trustedGithubActions` entry added in the **same commit** that adds the action —
not as a follow-up fix once Plumber blocks the PR. This is a standing convention, not a one-off
fix: the control is doing its job correctly each time (not a false positive), so the fix is in
contributor workflow, not in the gate.

## Consequences

**Easier:** No more PR-blocked-then-fixed round trips for a known, recurring class of surprise.

**Harder:** Requires remembering this convention when adding any new workflow step that references
a third-party action — nothing currently automates the reminder (e.g. no lint rule cross-checking
`uses:` lines in `.github/workflows/*.yml` against `.plumber.yaml`'s allowlist before a PR is
opened). Worth revisiting as tooling if a third occurrence happens.
