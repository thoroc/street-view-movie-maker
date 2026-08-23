# Scenario 03: Build Fails on a Broken Link

## User Prompt

"The docs CI job just went red on main. Can you figure out why and fix it?"

## Input

CI error output:

```text
$ npx @docmd/core build
ERROR: ENOENT: no such file or directory
  referenced from: docs/runbook/alarms.md:42
  target: docs/runbook/alarm-catalogue.md
```

Repository state: `docs/runbook/alarm-catalogue.md` was renamed to
`docs/runbook/alarm-catalog.md` in the most recent merge, but a link in
`docs/runbook/alarms.md` still points at the old filename. The built `site/`
directory from a stale local run still exists on disk with the old link baked in.

## Expected Behavior

1. Run the build check to reproduce the CI failure locally.
2. Read the ENOENT error and identify the broken reference: `docs/runbook/alarms.md:42` pointing at a file that no longer exists.
3. Fix the source markdown (`docs/runbook/alarms.md`) to point at the renamed `alarm-catalog.md`, not edit the stale `site/` copy.
4. Re-run the build to confirm it now exits 0.
5. Confirm the page count after the successful build is still in the expected 45-55 range (a sudden drop would signal missing content, e.g. if the rename silently dropped a page).
6. Report the root cause (stale rename) and the fix, ready to commit and push.

## Success Criteria

- Build check run before any other diagnosis step.
- Root cause correctly identified from the ENOENT path in the error, not guessed.
- Fix applied to `docs/runbook/alarms.md`, never to `site/`.
- Build re-run and confirmed to exit 0 after the fix.
- Page count sanity-checked post-build.

## Failure Conditions

- The stale `site/` directory is edited directly to "fix" the broken link.
- The fix is applied without first reproducing the failure locally.
- No re-run to confirm the build actually passes after the edit.
- Page-count check skipped entirely.
