---
title: "ADR-009: Adapt copied CI workflows to this repo's Rust/mise/hk stack"
status: accepted
date: 2026-08-23
context:
  - path: ".context/plans/2026-08-23-adapt-copied-github-workflows-to-rust-cli.md"
---

**Status:** Accepted
**Date:** 2026-08-23

## Context

Four `.github/workflows/` files (`ci.yml`, `deploy.yml`, `ai-hygiene.yml`, `plumber.yml`) were
copied from other projects and assumed tooling this repo doesn't have (a bun/Astro JS stack, a
docs-site build, `ctxharness`, `scripts/plumber-*.sh` + `.plumber.yaml`). A working reference,
`thoroc.github.io`, turned out to have the same CI-tooling profile this repo wants (mise+hk, aislop,
ctxharness, Plumber) minus the JS-specific bits, which resolved most of the adaptation as
"port verbatim" rather than "draft from scratch."

## Decision

- **`deploy.yml`: delete.** No docs-site source exists in this repo; re-add only when a real docs
  build lands.
- **`plumber.yml`: port the full subsystem** (three scripts + `.plumber.yaml`), preserving the
  gating model — Critical findings block the PR, High/Medium/Low file into a rolling backlog issue,
  plus a per-PR compliance comment. `.plumber.yaml` right-sized to this repo's actual profile
  (GitHub-only, no containers, ruleset-protected `main`, exactly two third-party actions in use:
  `jdx/mise-action` and `getplumber/plumber`) rather than porting the source project's full config
  verbatim.
- **`ai-hygiene.yml`: run both aislop and ctxharness**, advisory-only for both (not blocking) — this
  repo has no aislop baseline yet and ctxharness has never run here, so gating from a cold start
  risks blocking unrelated PRs on a first-run false positive. Promote to blocking once each has run
  clean for a while (tracked as a tech-debt/risk-register item once live).
- **`ci.yml`: adapt to this repo's actual mise+hk pipeline** (`mise install`, `hk check -a`,
  `cargo build`, `cargo test`) rather than either the original bun/astro file or a bare-rustup
  alternative — matches this repo's existing mise+hk standardization. `cargo-audit` included,
  advisory-only (`continue-on-error: true`).
- **`branchMustBeProtected` stays disabled in `.plumber.yaml`**, with a comment explaining why:
  this repo protects `main` via a GitHub ruleset, which Plumber's classic-branch-protection-only
  collector can't see (a known false-positive class), confirmed via the GitHub API rather than
  assumed.

## Consequences

**Easier:** CI now actually builds/tests this repo's real stack instead of failing on missing
bun/Astro tooling; the Plumber supply-chain gate is live and correctly scoped to what this repo
actually references.

**Harder:** `plumber.yml`'s Critical-gate is a real blocking gate once merged — a false-positive
Critical finding blocks all PRs until triaged, so its first real run should be checked against this
repo's own history before being trusted blindly. `ai-hygiene.yml`'s advisory-only posture means
aislop/ctxharness findings can accumulate without blocking anything until someone promotes them.

## Outcome

Merged as PR #2. First real CI run surfaced three environment gaps not anticipated by this decision
(none about the workflow files themselves, all pre-existing `mise.toml`/`mise.lock` issues never
exercised on Linux before): missing `linux-x64` lockfile entries for `age`/`fnox` (fixed via
`mise lock`), `ffmpeg`'s asdf plugin needing `nasm` on a fresh runner (fixed by switching to
`conda:ffmpeg`, a prebuilt binary), and `rust`'s `clippy`/`rustfmt` components not installed by
default on a clean runner (fixed by pinning `components = ["clippy", "rustfmt"]`). All four checks
pass as of the merge commit.
