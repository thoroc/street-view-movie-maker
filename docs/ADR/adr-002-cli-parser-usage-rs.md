---
title: "ADR-002: Use usage-rs (jdx/usage) instead of clap for CLI argument parsing"
status: accepted
date: 2026-08-22
context:
  - path: .context/plans/2026-08-22-port-street-view-movie-maker-to-rust-cli.md
  - path: .context/findings/2026-08-23-adr-candidates-from-python-to-rust-port-git-history.md
---

**Status:** Accepted
**Date:** 2026-08-22

## Context

The project already adopted `mise` and `hk` (jdx tooling) for its dev toolchain. `clap` is the conventional,
battle-tested choice for Rust CLI argument parsing. `usage`/`usage-rs` (jdx's own derive framework) was proposed
instead: the same `#[derive(Cli)]` struct that defines the flags also generates shell completions (bash/zsh/fish/
PowerShell/nushell), markdown docs, and a man page, and it's a lighter/faster parser than clap. Its own docs state
it is explicitly experimental ("point releases may break").

## Decision

Use `usage-rs` for CLI parsing, with the exact version pinned in `Cargo.toml`. If it proves too unstable during
implementation or afterward, fall back to `clap`, using the CLI flag table in the port plan as the source of truth
(no pipeline logic depends on which parser produces the struct).

## Consequences

Gains: one struct definition drives argument parsing, `--help` text, shell completions, and generated docs/man
pages, with a lighter parser than clap. Cost: every flag definition (`main.rs`'s `#[derive(Cli)]` struct) is coupled
to an experimental framework — a breaking point release could require an unplanned migration. This is a cross-
cutting decision, not a simple version pin: switching parsers later touches every flag definition and would need to
manually reproduce the generated completions/docs/man-page behavior another way.
