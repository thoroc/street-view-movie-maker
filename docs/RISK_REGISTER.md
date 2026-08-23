# Risk Register

A single committed table of every deferred item, interim shortcut, and open architecture decision for this project
that could otherwise get forgotten between sessions. Unlike gitignored, session-local scratch notes, this is a
durable, shared, GitHub-visible record.

| # | Item | Type | Description | Risk if unaddressed | Added | Status | Date | Decision |
| - | ---- | ---- | ----------- | -------------------- | ----- | ------ | ---- | -------- |
| 1 | `docs-check` skill targets a docs site this repo doesn't have | Open Decision | The `docs-check` skill (`.claude/skills/docs-check/`) validates a GitHub Pages documentation site built by `@docmd/core` -- build verification, orphan detection, ADR index freshness. This repo has no `@docmd/core` config, no GitHub Pages site, and `docs/` contains only `GETTING_STARTED.md`. It was imported wholesale alongside the rest of `.claude/skills/` and never adapted or removed. | An agent invoked for a docs task may try to run `docs-check`, fail against a site that doesn't exist, and waste a session diagnosing a "broken" build that was never built in the first place. | 2026-08-23 | Open | -- | -- |
