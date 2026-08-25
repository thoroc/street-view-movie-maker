# Scenario 03: A Real Decision Still Needs Its Own Finding

## User Prompt

"Is there anything to be learned from https://github.com/example-org/fast-polyline-simplify? I
think this one might actually be worth adopting."

(Assume the investigation confirms a genuinely portable technique svmm should adopt -- a faster
polyline-simplification algorithm directly applicable to the existing route-fingerprinting code.)

## Expected Behavior

1. Log an entry with `verdict: PORT_CANDIDATE` and a `reasoning` field explaining why it's a
   stronger signal than `NOTHING_NEW`/`INFORMS_EXISTING`.
2. Do not let the log entry substitute for the finding or plan this real decision actually needs --
   file a proper finding (or plan) via `context-file`/`plan-create` describing the technique and
   the adoption recommendation.
3. Point the log entry at the new finding/plan via its `related` field, once the finding/plan
   exists -- the log entry records that the investigation happened and where the real decision
   lives, not the decision's substance itself.
4. Run `scripts/validate-repo-scouting-log.sh` before treating the entry as logged.

## Success Criteria

- A `PORT_CANDIDATE` entry is logged with a `related` path to a newly created finding/plan.
- The finding/plan itself (not just the log entry) documents the actual technique and
  recommendation in enough detail for someone else to act on it.
- The log entry's `reasoning` stays a short pointer/summary, not a substitute for the finding's
  full detail.
- Validation is run before declaring the entry logged.

## Failure Conditions

- A `PORT_CANDIDATE` verdict is logged with all of the real decision's substance stuffed into the
  log entry's `reasoning`, and no separate finding or plan is ever filed.
- `related` is left empty even though a finding was created (or vice versa: `related` points at
  something that was never actually written).
- The decision is only described in chat, with no durable finding/plan or log entry at all.
