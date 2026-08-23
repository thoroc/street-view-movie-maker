# Value Rubric and Read Protocol

The `value` frontmatter field records the **benefit-of-action** of a `.context/` action-candidate entry: how much doing
it unblocks or leverages future work. It is one of three distinct axes, and must not be conflated with the other two:

| Axis              | Question it answers               | Field                    |
| ----------------- | --------------------------------- | ------------------------ |
| Benefit-of-action | How much good does doing this do? | `value`                  |
| Cost-of-action    | How much work is it?              | `effort` (plans only)    |
| Risk-of-inaction  | How bad is leaving it undone?     | `severity` (issues only) |

`value` applies to the three action-candidate types: `PLAN`, `FINDING`, and `KNOWN_ISSUE`. It does not apply to
`ANALYSIS`, `INSTRUCTION`, or `AUDIT`, which are reference material rather than things to do next.

This rubric is the standard that all `value` grades are assigned against. Author grades before the sort trusts them:
`value` is an authoritative sort key (see the read protocol below), not an advisory label.

## Grading criteria

Grade against three questions, in priority order. When they disagree, leverage dominates.

1. **Leverage** — Does completing this unblock or simplify other work, or is it a leaf that helps only itself?
   High-leverage items are foundational: other work depends on them, or they retire a class of recurring effort.
2. **Consumers unblocked** — How many future work items or people can proceed once this lands? Count concrete
   downstream dependents, not hypotheticals.
3. **Reversibility and decay** — Cheap-to-reverse, low-decay work is safer to rate up. Work whose benefit evaporates if
   delayed (a time-boxed fix, a grade that goes stale) may warrant a higher grade to capture the closing window.

### `HIGH`

Foundational or broadly-leveraged: several downstream items depend on it, or it retires a recurring cost, or it closes a
gap that keeps re-manifesting. Doing it changes what else becomes possible.

### `MEDIUM`

Clear standalone benefit with limited leverage: it improves one workflow, closes one gap, or unblocks one or two
consumers, but nothing else is waiting on it.

### `LOW`

Narrow, self-contained, or nice-to-have: benefits a single consumer, is easily deferred, or is polish rather than
capability. Correct to do eventually, not urgent to do next.

## Read protocol

`value` is an **authoritative sort key**, not an advisory label. To answer "which item is highest value to do next?",
read `.context/index.yaml` and:

1. Filter to `status` in {`DRAFT`, `READY`, `ACTIVE`, `DEFERRED`} of type `PLAN`, `FINDING`, or
   `KNOWN_ISSUE`. (`DONE`/`SUPERSEDED` grades exist as a learning corpus and never enter this sort.
   `READY` only ever appears on `PLAN` entries — `FINDING`/`KNOWN_ISSUE` don't use the ready/active
   split; see `planning-flow.md`.)
2. **Drop any `DEFERRED` item whose `deferred_until` is a future date** (strictly after today): it is not listed at all
   until that date arrives. `deferred_until` governs visibility even when the item is also externally blocked — the date
   takes precedence over the "blocked but visible" default (an item can be both; the date wins). Items with no
   `deferred_until`, or whose `deferred_until` has passed, remain candidates.
3. Split the survivors into four tiers and always exhaust an earlier tier before the next:
   **tier 1** = `ACTIVE` (implementation actually underway — the most immediately useful to look
   at); **tier 2** = `READY` (approved and queued, not yet started); **tier 3** = `DRAFT` (not yet
   reviewed); **tier 4** = `DEFERRED` (real but not actionable yet — date-gated or externally
   blocked; _not_ merely low-priority, which is `value: LOW` on an `ACTIVE`/`READY` item). A
   `DEFERRED` item never outranks an `ACTIVE`/`READY`/`DRAFT` one, regardless of its `value`. A
   `DEFERRED` item whose `deferred_until` has passed (so it survived step 2) is
   reactivation-eligible — surface it for promotion to `READY` rather than leaving it parked.
4. Within each tier, sort by `value` descending (`HIGH` > `MEDIUM` > `LOW`).
5. Break ties by `effort` ascending (`S` < `M` < `L` < `TBD`) where present. Findings and known-issues have no `effort`,
   so within a bucket they sort by `value` alone.
6. Break any remaining tie by `themes[0]` (the primary theme): prefer the item whose primary theme matches the area
   already in focus. Theme expresses preference-of-area, not priority, so it sits below both magnitude axes. See
   [`theme-vocabulary.md`](theme-vocabulary.md).
7. Act on the top item **without re-forming an independent judgement**. Relocating the judgement to read-time would
   reopen the gap this field closes. Before picking a `DEFERRED` item, confirm its blocker has cleared and reactivate it
   to `READY`; if the whole `ACTIVE`/`READY`/`DRAFT` set is empty and every `DEFERRED` item is still blocked, there is
   genuinely nothing to pick up.

This protocol only holds if the grades are trustworthy. That is why grading against this rubric (not ad hoc), the
calibration pass on backfill, and re-grading on status transitions are load-bearing, not optional.
