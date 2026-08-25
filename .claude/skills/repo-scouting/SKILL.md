---
name: repo-scouting
description: "Log the outcome of investigating an external GitHub repo for techniques street-view-movie-maker could learn from or port, so a future session doesn't re-investigate the same repo from scratch. Use after investigating ANY external repo for portable techniques or features, whether or not anything was found -- a null result is exactly the case this exists to record. DO NOT use for a decision that's actually been made (that still gets its own finding/plan via context-file, this log just points at it), for internal svmm code review, or for anything confidential. Triggers: 'check the repo scouting log', 'log this investigation', 'have we already checked this repo', 'record this as nothing new', 'log a repo scouting entry'."
---

# Repo Scouting

A single append-only log (`.context/repo-scouting/log.jsonl`) recording every external GitHub repo investigated for
techniques `svmm` could learn from or port, and what was decided. Built after a session investigated ~10 repos and
discovered its null results ("nothing new here") were never recorded anywhere -- a future session had no way to know
a repo had already been checked short of re-reading the chat transcript. See
`.context/plans/2026-08-25-add-repo-scouting-log-skill.md` for the full design rationale, including why the log stays
gitignored under `.context/` rather than a committed `docs/` file, and why the format is JSONL rather than YAML.

## Rules

1. **Log every investigation, not just the interesting ones.** A `NOTHING_NEW` verdict is exactly as valuable to
   record as a `PORT_CANDIDATE` one -- it's the difference between "we haven't looked" and "we looked and it wasn't
   useful," and only the log distinguishes them for a future session.
2. **This is a research log, not a decision registry.** It does not replace `docs/RISK_REGISTER.md` or
   `docs/TECH_DEBT.md`. An investigation that produces a real decision (e.g. "port streetlevel's Baidu module to
   Rust") still gets its own finding or plan via `context-file`/`plan-create` as normal -- the log entry's `related`
   field just points at it once it exists.
3. **No automatic enforcement exists for this log.** `.context/` is gitignored project-wide, so no pre-commit hook
   can ever see this file -- unlike `risk-register`/`tech-debt`'s committed docs, nothing stops a session from
   skipping this skill entirely. Checking the log before investigating, and logging afterward, are conventions this
   skill documents, not gates git or `hk` enforce.
4. **The log lives in one worktree at a time.** `.context/` is untracked, so each git worktree has its own
   independent copy. After logging an entry in a worktree, sync `.context/repo-scouting/log.jsonl` back to the main
   checkout the same way any other `.context/` file is synced (see `ways-of-working.md`) -- otherwise the entry is
   invisible to sessions working from the main checkout, and is lost entirely if the worktree is later removed.

## Prerequisites

- `.context/repo-scouting/log.jsonl` -- created on first use if it doesn't exist yet (an empty/missing file is not an
  error; see Workflow)
- `assets/templates/repo-scouting-entry.yaml` -- the shape of a single entry, illustrated (with a worked example
  showing both the annotated YAML and the actual single-line JSON form the log uses)
- `assets/schemas/repo-scouting-entry.schema.json` -- the same shape, as enforceable constraints
- `scripts/validate-repo-scouting-log.sh` -- validates the whole log's schema conformance and checks for duplicate
  `url` values; no pre-commit wiring (Rule 3)
- `jq` (schema constraints are read at runtime, never hardcoded -- see Scripts), pinned in `mise.toml`

## When to Use

- Before investigating an external repo for portable techniques or features: check `.context/repo-scouting/log.jsonl`
  for an existing entry with the same `url` first.
- After investigating any external repo, regardless of outcome: append an entry.
- Someone asks whether a specific repo has already been checked, or what's been scouted so far.

## When NOT to Use

- **A decision has actually been made** (a technique is being ported, a provider is being adopted) -- that's a
  finding or plan via `context-file`/`plan-create`; log an entry here that points at it via `related`, don't
  substitute the log entry for the finding/plan itself.
- **Internal code review or refactoring analysis of `svmm`'s own code** -- this skill is for *external* repos only.
- **Anything confidential** -- `.context/` is gitignored but not encrypted; treat it the same as any other
  `.context/` file for sensitivity purposes.

## Workflow

### Checking before investigating

1. If `.context/repo-scouting/log.jsonl` doesn't exist, there's nothing logged yet -- proceed with the investigation.
2. Otherwise, check whether an entry's `url` already matches (case-insensitively) the repo about to be investigated:

   ```bash
   grep -i "github.com/OWNER/REPO" .context/repo-scouting/log.jsonl
   ```

3. If a match exists, read its `verdict` and `reasoning` before deciding whether to re-investigate. A `NOTHING_NEW`
   or `INFORMS_EXISTING` entry from months ago on a repo that's since had major changes may still warrant a fresh
   look -- the log records what was true when it was checked, not a permanent verdict.

### Logging an entry

1. Investigate the repo as normal.
2. Compose an entry matching `assets/schemas/repo-scouting-entry.schema.json`'s shape (`url`, `date`, `repo_name`,
   `summary`, `verdict`, `reasoning`, optional `related`) -- see `assets/templates/repo-scouting-entry.yaml` for a
   worked example, including the exact single-line JSON form.
3. Choose `verdict` from: `NOTHING_NEW` (different domain, or a strict subset of prior art already on file),
   `INFORMS_EXISTING` (added a note/section to an already-open finding or known-issue), `NEW_FINDING_NEEDED` (novel
   enough to warrant its own new `.context/` file, not yet filed), `PORT_CANDIDATE` (a stronger signal, actually
   recommended for implementation).
4. Append the entry as a single line to `.context/repo-scouting/log.jsonl` (create the file, and the
   `.context/repo-scouting/` directory, if this is the first entry).
5. Run `scripts/validate-repo-scouting-log.sh` to check for zero errors before treating the entry as logged -- catches
   a malformed entry or an accidental duplicate immediately rather than leaving it for the next reader to notice.
6. If working in a worktree, sync the log back to the main checkout (Rule 4).

## Mindset

- A `NOTHING_NEW` entry is not a wasted write -- it's the only thing that turns "we haven't looked" into "we looked
  and it wasn't useful" for the next session. Treat logging as part of finishing the investigation, not an optional
  extra step after the real work is done.
- This log's value compounds only if every investigation is logged, not just the ones that felt interesting. A log
  that only records `PORT_CANDIDATE`/`NEW_FINDING_NEEDED` verdicts is indistinguishable from no log at all on the
  question that actually matters: "has anyone checked this repo before?"
- The log describes what was true when it was checked, not a permanent verdict -- a repo that changes substantially
  after a `NOTHING_NEW` entry may warrant a fresh look, not an automatic skip.

## Scripts

One check -- schema conformance and duplicate-URL detection together, since both are cheap to run in a single pass
over the log:

```bash
scripts/validate-repo-scouting-log.sh   # Content: does every line match repo-scouting-entry.schema.json, and are all urls unique?
```

`related` path existence is checked but only warns (never fails) -- a path can legitimately point at a finding/plan
not written yet.

## Anti-Patterns

**NEVER** skip logging a `NOTHING_NEW` result because it doesn't feel worth recording.
**WHY:** The whole value of this log is distinguishing "we haven't looked" from "we looked and it wasn't useful" --
skipping the boring verdicts leaves exactly the case a future session needs most.
**BAD:** Investigating a repo, concluding it's unrelated, and moving on without appending an entry.
**GOOD:** Appending a `NOTHING_NEW` entry with a one-line reason, even when the investigation took two minutes.

**NEVER** treat a logged entry as a permanent verdict that blocks re-investigation forever.
**WHY:** The log records what was true when it was checked -- a repo that's changed substantially since a
`NOTHING_NEW` entry may deserve a fresh look, and treating old entries as immutable defeats that.
**BAD:** Refusing to look at a repo again just because any entry exists for its URL, regardless of age or content.
**GOOD:** Reading the existing entry's `verdict` and `reasoning` first, then deciding whether it still holds.

**NEVER** let a log entry substitute for the finding or plan a real decision needs.
**WHY:** This is a research log, not a decision registry -- a `PORT_CANDIDATE`/`NEW_FINDING_NEEDED` verdict with no
follow-up finding leaves the actual decision undocumented anywhere durable.
**BAD:** Logging `verdict: PORT_CANDIDATE` with reasoning in the log entry and stopping there.
**GOOD:** Filing the finding/plan via `context-file`/`plan-create`, then pointing the log entry at it via `related`.

**NEVER** forget to sync a worktree's log back to the main checkout before the worktree is removed.
**WHY:** `.context/` is untracked, so a worktree's copy of the log is the only copy that exists until synced --
removing the worktree first destroys the entry permanently, with no git history to recover it from.
**BAD:** Logging an entry in a worktree, then calling `ExitWorktree` with `action: "remove"` without syncing first.
**GOOD:** Copying `.context/repo-scouting/log.jsonl` back to the main checkout as part of finishing the session's
work, the same way any other `.context/` file created in a worktree is synced.

## References

| Topic | Reference | When to Use |
| --- | --- | --- |
| Entry shape as illustrated YAML plus its actual JSONL form | `assets/templates/repo-scouting-entry.yaml` | Drafting a new entry before appending it |
| Entry shape as enforceable constraints | `assets/schemas/repo-scouting-entry.schema.json` | Checking what the validator script actually enforces |
| Full design rationale (location, format, verdict enum, deferred risks) | `.context/plans/2026-08-25-add-repo-scouting-log-skill.md` | Understanding why this skill is shaped the way it is |
| Deciding whether an investigation's outcome belongs in `docs/RISK_REGISTER.md` or `docs/TECH_DEBT.md` too | `risk-register` / `tech-debt` skills | An investigation surfaces a real cost, risk, or cleanup item beyond "log the verdict" |
| One worked example per `verdict` value, and the full troubleshooting table | `references/worked-examples-and-troubleshooting.md` | Unsure how much detail an entry needs, or a validation run fails and the fix isn't obvious |
