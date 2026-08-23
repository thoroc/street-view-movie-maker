# Scenario 04: LLM Output Files Missing After Build

## User Prompt

"We enabled the llms.txt export last sprint -- can you confirm it's actually working before the release?"

## Input

`docmd.config.json` excerpt:

```json
{
  "llms": { "enabled": true }
}
```

After running the build, `site/` contains:

```text
site/
  index.html
  architecture/index.html
  runbook/index.html
  (no llms.txt, llms-full.txt, or llms.json)
```

## Expected Behavior

1. Run the build check first.
2. Run the LLM output audit: check `site/llms.txt`, `site/llms-full.txt`, and `site/llms.json` exist, and count entries in `llms.txt`.
3. Discover all three files are missing despite `llms.enabled: true` in the config.
4. Do not report this as a docs-check pass just because the build itself succeeded -- a successful build with `llms.enabled: true` and no LLM output files is a distinct failure this skill exists to catch.
5. Escalate correctly: this is a build/config issue to investigate (e.g. config not picked up, wrong docmd version), not a content-quality issue for `markdown-authoring`.
6. Report the discrepancy clearly: config says enabled, output says otherwise.

## Success Criteria

- Build check run before the LLM audit.
- All three expected files explicitly checked for existence (not just one).
- Discrepancy between config and actual output surfaced explicitly.
- Not misclassified as a `markdown-authoring` or content-quality concern.
- Recommends a concrete next step (e.g. re-check docmd version/config wiring) rather than only stating "it's broken".

## Failure Conditions

- Build success alone is treated as sufficient sign the LLM export works.
- Only one of the three files is checked, missing that all three are absent.
- The issue is redirected to `markdown-authoring` instead of being treated as a build/config gap.
- No entry-count check attempted for `llms.txt`.
