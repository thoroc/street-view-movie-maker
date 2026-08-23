# Scenario 01: New Page Added Without an Index Link

## User Prompt

"I just added `docs/architecture/deployment-flow.md` explaining the deploy pipeline. Can you check the docs are in good shape before I push?"

## Input

Repository state:

```text
docs/
  index.md                       # does not mention deployment-flow.md anywhere
  architecture/
    deployment-flow.md           # new file, 120 lines, well-written content
    system-map.md                # existing, linked from docs/index.md
docmd.config.json                # llms.enabled: true
```

`docs/index.md` excerpt:

```markdown
## Architecture
- [System Map](architecture/system-map.md)
```

## Expected Behavior

1. Run the build check (`npx @docmd/core build`) first, before any other check.
2. Run orphan detection and discover `docs/architecture/deployment-flow.md` is not referenced from `docs/index.md` or any subsection index.
3. Fix the source: add a link to `deployment-flow.md` under the `## Architecture` section of `docs/index.md`, not by editing anything under `site/`.
4. Re-run orphan detection to confirm the page is no longer orphaned.
5. Do not stop at "build passed" -- a passing build does not imply the new page is discoverable.
6. Report the fix made and confirm no other orphans exist before recommending push.

## Success Criteria

- Build check run before other checks.
- Orphan correctly identified via the orphan-detection command (not guessed).
- Fix lands in `docs/index.md` (the source), never in `site/`.
- Orphan detection re-run and confirmed clean after the fix.
- Final report states the site is ready to push, or lists remaining issues.

## Failure Conditions

- Orphan page reported as fine because the build succeeded.
- Fix applied to the built `site/` output instead of `docs/index.md`.
- Orphan detection command skipped in favour of manually eyeballing the index.
- No re-check after the fix to confirm the orphan is resolved.
