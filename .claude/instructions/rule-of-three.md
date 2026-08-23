# Rule of Three: promote repeated workflows to tools

When you find yourself assembling the same multi-step, ad-hoc workflow by hand, stop and check whether it should become a real tool (a script, a skill, or a hook) instead of being rebuilt from scratch
again.

## The rule

- **First time:** just do it.
- **Second time:** do it again, but note the repetition.
- **Third time (or more):** stop. Propose promoting the workflow to a tool.

"Three" is a threshold, not a law. Use judgement: an expensive or error-prone sequence may warrant promotion on the second occurrence.

## The cross-session trap

The dangerous case is reassembling a workflow that was **already built temporarily in an earlier session**. A fresh session has no memory of the last one, so its internal counter starts at one and
never trips the threshold. To avoid this, the count must be checked against persistent history, not just the current conversation.

## Procedure

Before hand-assembling a multi-step workflow (a chain of CLI calls, a bespoke formatting or migration sequence, a repeated investigation pattern):

1. **Check history first.** Search persistent memory for prior occurrences before rebuilding:
   - `grep -n '\[tasks\.' mise.toml` for a mise task that already covers it — cheaper and more authoritative than
     reassembling a shell pipeline from scratch.
   - `mem-search` / claude-mem `observation_search` for "did we do this before".
   - `rtk discover` to surface recurring command sequences in Claude Code history ("missed opportunities").
   - `qmd` over the journal and plans for a prior write-up — a prior ticket's entry will often name the tool it used even when the tool itself isn't named after the current task's keywords.
2. **If it has been done before**, treat this as at least the second occurrence. Do not silently rebuild it.
3. **On the third occurrence**, stop and propose promotion: a `mise.toml` task for a command, a `.claude/skills/` skill for a workflow, or a hook for an automatic trigger. Capture it as a plan
   under `.context/plans/` if it is non-trivial.
4. **Record the occurrence** so the count survives the session: add a claude-mem observation noting the workflow and that it was assembled by hand. The next session can then find it.

## Do not build a bespoke tracker

Tracking repetition is itself a workflow, so the rule applies to it too. Use the existing substrate (claude-mem observations, `rtk gain --history` / `rtk discover`, `qmd`) rather than writing a new
activity-tracking tool. Only build something new here if those genuinely cannot answer "have we done this before".

## Worked example

None yet in this repo — record one here the first time a hand-assembled workflow gets promoted to a `mise.toml` task, skill, or hook, so future sessions have a concrete precedent instead of a
hypothetical.
