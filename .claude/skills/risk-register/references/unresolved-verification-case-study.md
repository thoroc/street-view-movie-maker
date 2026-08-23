# Case Study: Verify "Unresolved" Before Filing

Row 11 (2026-07-31) is the concrete case that motivates the "confirm this is genuinely unresolved" step in the
Workflow section of `SKILL.md`.

## What happened

Row 11 was filed as an "Open Decision" about whether a session-local scratch-notes directory should be committed to
the repository. At the time it was written, the project's own instructions already settled this question: the
planning-flow instructions doc (kept alongside this project's other agent instructions) already stated that the
scratch-notes directory is deliberately gitignored working scratch, not something to commit.

The row was wrong the moment it was written -- not because the underlying observation was false (there genuinely was
a discrepancy worth noticing), but because the "is this already decided?" check never happened before filing.

## The lesson

A discrepancy you have not seen before is a candidate for a register row, not a verdict that it belongs there. Before
filing:

1. Search the project's root-level agent guidance file and its instructions directory for an existing rule or
   convention that already settles the question.
2. Check any doc the item touches for a stated decision.
3. Only file the row once you have confirmed no existing convention already answers it.

This is the same discipline as verifying a pr-agent finding against the actual code before trusting it (see the
`mr-review` skill) -- a claim of "this is unresolved" needs the same scrutiny as a claim of "this is a bug." Skipping
the check produces a register that looks authoritative but contains rows that were never actually open questions.

**When to use this reference:** before filing a new "Open Decision" row, especially one that feels like a surprising
gap -- confirm it is not already settled somewhere in the project's own documentation first.
