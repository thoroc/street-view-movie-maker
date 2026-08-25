# Repo Scouting — check before, log after

Before investigating an external GitHub repo for techniques `svmm` could learn from or port, check
`.context/repo-scouting/log.jsonl` for an existing entry with the same URL (case-insensitively) — a repo may have
already been checked, with a verdict and reasoning already recorded. After investigating, always log an entry, even
when the outcome is "nothing new" — a null result is exactly what this log exists to preserve, since without it a
future session (or this one, after `/clear`) has no way to tell "we already checked this" from "we haven't looked
yet" short of re-reading a chat transcript.

Use the `repo-scouting` skill for the full workflow, entry schema, and validation script. See
`.context/plans/2026-08-25-add-repo-scouting-log-skill.md` for the design rationale.

This is a convention, not an enforced gate: `.context/` is gitignored, so no pre-commit hook can ever see this log —
nothing stops a session from skipping this instruction entirely.
