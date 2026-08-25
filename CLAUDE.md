# street-view-movie-maker

`svmm` is a Rust CLI that turns a route (point A to point B) into a movie made of Google Street View images, using
the Directions API for the route and the Street View Static API for frames, encoded into video by a native Rust
encoder (no `ffmpeg` dependency; see [ADR-013](docs/ADR/adr-013-native-rust-codec-pure-rust-default-opt-in-h264.md)).
See [README.md](README.md) and [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) for setup and usage. The repo is
hosted on GitHub; use `gh` for PRs, issues, and checks.

This file wires in the standing instructions under `.claude/instructions/`. They are not auto-loaded by Claude Code
on their own — this file is what makes them part of every session.

## Ways of working

@.claude/instructions/ways-of-working.md

## Planning flow (before non-trivial changes)

@.claude/instructions/planning-flow.md

## The `.context/` governance system

Plans, findings, and other `.context/` files use the frontmatter contract, effort/value/severity grading, and theme
vocabulary defined here. Read these before creating or grading a `.context/` file — the `context-file`/`plan-create`
skills reference them too, but the rubric and vocabulary live here, not in the skill.

@.claude/instructions/value-rubric.md

@.claude/instructions/theme-vocabulary.md

@.claude/instructions/follow-up-triage.md

@.claude/instructions/rule-of-three.md

@.claude/instructions/skill-authoring.md

@.claude/instructions/repo-scouting.md

## Rust code style and maintainability

@.claude/instructions/rust-style.md

## Tooling

@.claude/instructions/rtk.md

@.claude/instructions/aislop.md

@.claude/instructions/code-review-graph.md

@.claude/instructions/context-mode.md

@.claude/instructions/qmd.md

@.claude/instructions/plain-english.md
