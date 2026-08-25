# Scenario 01: Log a Null Result

## User Prompt

"Is there anything to be learned from https://github.com/example-org/some-map-widget in the
context of our project?"

(Assume the investigation has already happened: the repo is a client-side map UI widget, a
different domain from `svmm`'s batch video pipeline, with nothing to port.)

## Expected Behavior

1. Report the investigation's outcome to the user honestly -- nothing new, different domain.
2. Do not skip logging this result just because it wasn't interesting -- append a `NOTHING_NEW`
   entry to `.context/repo-scouting/log.jsonl` anyway, since a null result is exactly what this
   log exists to record.
3. The logged entry includes `url`, `date`, `repo_name`, `summary`, `verdict: NOTHING_NEW`, and a
   `reasoning` field that explains *why* it's a null result (domain mismatch), not just that it is
   one.
4. Note in the entry (or in the response) that this reflects today's investigation, not a
   permanent verdict -- if the repo changes substantially later, it may warrant another look.
5. Run `scripts/validate-repo-scouting-log.sh` before treating the entry as logged.

## Success Criteria

- A `NOTHING_NEW` entry is actually appended to the log, not just described in chat.
- `reasoning` is self-contained and specific (names the domain mismatch), not a generic "nothing
  found."
- The entry or response acknowledges this is not a permanent verdict.
- Validation is run (or its command shown) before declaring the entry logged.

## Failure Conditions

- The investigation is reported to the user but never logged, because the result "wasn't worth
  recording."
- `reasoning` is empty or generic ("nothing useful") with no specifics.
- The entry is presented as a permanent, unrevisitable verdict.
- Validation step skipped entirely.
