# Scenario 02: Check the Log Before Investigating, Then Sync

## User Prompt

"Is there anything to be learned from https://github.com/sk-zk/streetlevel? I think we may have
looked at this before."

(Assume `.context/repo-scouting/log.jsonl` already contains an `INFORMS_EXISTING` entry for this
exact URL from a prior session, and the current session is working inside a git worktree.)

## Expected Behavior

1. Check `.context/repo-scouting/log.jsonl` for an existing entry matching this URL
   (case-insensitively) before launching a fresh investigation.
2. Find the existing entry and read its `verdict` and `reasoning` back to the user, rather than
   re-investigating the repo from scratch.
3. Only re-investigate if there's a specific reason to think the repo has materially changed since
   the logged date -- otherwise, treat the existing entry as still current.
4. If a new entry does end up being logged during this session (e.g. because the repo did change),
   sync `.context/repo-scouting/log.jsonl` back to the main checkout before the worktree is
   removed, the same way any other `.context/` file created in a worktree is synced.

## Success Criteria

- The log is checked for a matching URL before any fresh investigation begins.
- The existing entry's verdict/reasoning is surfaced to the user.
- No redundant re-investigation happens without a stated reason.
- If the worktree's log is touched, the response mentions or performs syncing it back to the main
  checkout before the worktree is removed.

## Failure Conditions

- The repo is re-investigated from scratch without ever checking the log first.
- An existing entry is found but ignored or not surfaced to the user.
- A worktree's new log entry is left unsynced with no mention of the sync step.
