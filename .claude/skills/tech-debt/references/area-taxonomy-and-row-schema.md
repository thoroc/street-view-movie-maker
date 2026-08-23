# Area Taxonomy and Row Schema

Full detail behind `docs/TECH_DEBT.md`'s `Area` column and the row schema as a
whole. Load this before filing a new row, or when unsure whether an item
needs a new `area` value.

## Area Taxonomy

`area` is a small, tech-debt-specific set of values, not a closed JSON Schema `enum` and not the `.context/` themes
vocabulary (`.claude/instructions/theme-vocabulary.md` is scoped to a different purpose and coarser than a typical
tech-debt item). Same "ship coarse, split on evidence" pattern that vocabulary already documents for its own six
themes, applied here to a second, independent taxonomy.

Current values (add a new one only when an item genuinely doesn't fit an existing one):

- `Code quality` -- lint/style violations, unsafe patterns, dead code. Does NOT cover the one-export-per-module
  convention -- that's `Module structure` below, split out once the evidence showed up (see Revisit trigger log).
- `Module structure` -- a file violates the one-exported-symbol-per-module convention
  (`.claude/instructions/typescript-standards.md` Rule 2): more than one exported function in a file, a helper
  function exported alongside the file's real function, or an exported constant co-located with the file's one
  function. No lint rule enforces this convention (it's manual, not a Biome rule), which is why it recurs across
  unrelated files without ever failing the build.
- `Testing` -- a test that doesn't actually exercise what it claims to (over-mocked, assertion doesn't tie to the
  behaviour under test), where the real regression risk is independently covered elsewhere. If nothing else covers
  the regression risk, that's `RISK_REGISTER.md`, not this doc -- see Rule 3 in `SKILL.md`.
- `Observability` -- a gap in logs, metrics, or diagnosability that makes it harder to tell why something failed,
  with no existing gate that would catch it.
- `Infrastructure` -- a leftover or misconfigured cloud/Terraform resource with no functional impact today but an
  ongoing cost or a stale dependency (an orphaned key, an unused resource left behind by a migration).
- `Tooling` -- a gap in this project's own development or audit tooling/process (not the product's runtime code),
  e.g. a missing freshness or consistency check in an internal script or an external tool integration.

**Revisit trigger**: if a single `area` value accumulates more than roughly 30% of open rows, split it (matching
`theme-vocabulary.md`'s own split-on-evidence threshold). AVOID splitting pre-emptively -- TYPICALLY one value covers
a project's entire early life; only split once the evidence (the 30% threshold) actually shows up.

**Trigger log**: `Code quality` hit the threshold on 2026-08-14 (5 of 8 open rows, ~63%) -- but all five turned out
to be the exact same convention violation applied to different files, not five distinct kinds of code-quality issue.
The split that restores discriminating power here isn't dividing one big bucket into several medium ones; it's
recognizing the whole cluster was mislabeled under a too-generic name and giving it the specific one (`Module
structure`) it actually needed. `Code quality` itself currently has zero rows again as a result -- that's expected,
not a sign the value should be removed; it stays available for the next lint/style/dead-code item that isn't this
specific pattern.

## Row Schema

| Column        | Meaning                                                                                          |
| ------------- | -------------------------------------------------------------------------------------------------- |
| `#`           | A simple row number, not a permanence claim -- freely reused after a row is deleted, since there's no audit requirement to anchor against (contrast `risk-register`'s append-only-backed `#`). |
| Item          | Short name, not a sentence.                                                                        |
| Area          | From the Area Taxonomy above.                                                                      |
| Description   | Self-contained: what's wrong, where, and why existing gates didn't catch it.                       |
| Effort        | `S` \| `M` \| `L` -- sizing only. Put a suggested fix approach in Description, not here.            |
| Since         | `YYYY-MM-DD`, the date the row was created.                                                        |
| Status        | `Open` \| `In Progress`. No `Resolved` -- a resolved row is deleted (Rule 1).                       |

## Worked example row

```yaml
"#": 2
Item: "Unused `LegacyRenderer` import in analysis/render-html.ts"
Area: "Code quality"
Description: >
  render-html.ts imports LegacyRenderer but no longer calls it since the
  ADR-029 prompt migration. eslint's no-unused-vars is warning-level here
  (not error), so it doesn't fail the build. Delete the import.
Effort: S
Since: "2026-07-31"
Status: "Open"
```

BY DEFAULT a new row starts `Status: Open`. PREFER `In Progress` only once
someone has actually started the fix, UNLESS the fix is small enough to land
in the same sitting as filing -- in which case just fix it and skip filing
the row at all.
