# Plain English for all human-facing output

Every response and document a human is meant to read — chat replies, journal entry prose, Jira/MR comment drafts, follow-up and plan write-ups — defaults to plain, direct English. This is a style
default, not a content restriction: technical terms, resource names, code identifiers, and ticket IDs stay exactly as precise as the subject requires. The goal is to cut unnecessary verbosity and
jargon-stacking, not to dumb down or omit detail that changes the reader's understanding.

## Scope

Applies to prose: chat responses, session summaries, journal entry narrative sections (Session Overview, Root Cause, Session Outcome, etc.), Jira/MR/PR comment drafts, follow-up and plan documents.

Does **not** apply to (leave these exact, unsimplified):

- Code, commands, config, and their output (fenced blocks)
- Log lines, error messages, and other quoted evidence
- Resource/identifier names (alarm names, ARNs, function names, file paths)
- Required structural sections and compliance checklists mandated by a project template (e.g. the journal-entry-creator schema)

## Rules

- **One idea per sentence.** Split stacked clauses joined by em-dashes, semicolons, or parentheticals into separate sentences when each clause is its own idea.
- **Common words over jargon.** Prefer the plain word when one exists ("found" not "identified"; "checked" not "validated" — unless "validated" is the precise technical claim being made).
- **Active voice, named actor.** "The alarm never fired" beats "it was determined that the alarm did not fire."
- **Lead with the fact, not the process of finding it.** State what's true, then (if needed) how it was confirmed — not the reverse.
- **Cut hedging padding.** Say what you found. Reserve qualifiers ("likely", "not confirmed") for genuine uncertainty, not as a verbal tic.
- **Chat replies specifically:** follow the global "short and direct" instruction in `~/.claude/CLAUDE.md` — lead with the answer, skip preamble, expand only when asked or when the task needs it.

## Relationship to other rules and skills

- The `documentation--plain-english` skill remains available for a deeper editing pass on a specific document (e.g. before posting a Jira comment or publishing a report) — invoke it explicitly when a
  document needs more than the defaults above.
- `~/.config/claude/rules/mr-pr-descriptions.md` already applies this same philosophy specifically to MR/PR `## What`/`## Why` sections — that rule's specifics (drop internal identifiers, one bullet
  one idea) still govern MR/PR descriptions; this file extends the same default project-wide.
- Do not apply ASD-STE100 or another controlled-vocabulary standard here — those restrict word choice for translation/safety-critical documentation and conflict with the technical precision this
  repo's investigations require (AWS resource names, code symbols, exact log text).

## Example

**Before:** "Following an extensive investigation traversing two generations of infrastructure-as-code, it was determined that the underlying root cause of the alerting gap was attributable to a
metric filter having never been provisioned against the alarm in question, which has consequently remained in an `INSUFFICIENT_DATA` state since its original creation."

**After:** "The alarm has had no metric filter feeding it since it was created in 2020. That's why it never fired."
