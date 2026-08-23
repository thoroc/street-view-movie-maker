# Scenario 02: ADR Added Without Registering in the Index

## User Prompt

"Check the docs before I open the MR -- I added a new ADR for the retry-sweep decision."

## Input

Repository state:

```text
docs/ADR/
  adr-017-delivery-record-schema.md    # registered
  adr-018-observability.md             # registered
  adr-026-yearly-cadence.md            # new, NOT registered
  index.yaml                           # last entry is adr: 018
```

`docs/ADR/index.yaml` excerpt:

```yaml
adrs:
  - adr: 017
    title: Delivery record schema
    status: accepted
  - adr: 018
    title: Observability
    status: accepted
```

Also present: `docs/ADR/adr-012-old-format.md.bak` -- a renamed-away file whose old
name (`adr-012-old-format`) is still listed in `index.yaml`.

## Expected Behavior

1. Run the ADR index freshness check (compare `docs/ADR/adr-*.md` against `docs/ADR/index.yaml` in both directions).
2. Find `adr-026-yearly-cadence.md` is on disk but missing from `index.yaml` -- register it, using the `adr-capture` skill to generate the entry rather than hand-writing YAML.
3. Find `adr-012-old-format` is listed in `index.yaml` but has no corresponding `.md` file on disk (only a `.bak`) -- flag it as a stale entry and remove it from the index or restore the file, per the anti-pattern rule.
4. Do not silently ignore the stale entry just because the user only asked about the new ADR.
5. Report both findings and the actions taken.

## Success Criteria

- Both directions of the reconciliation are checked (unregistered ADRs AND stale index entries).
- `adr-026-yearly-cadence` gets added to `index.yaml`, via `adr-capture` rather than a manual guess at the schema.
- The stale `adr-012-old-format` entry is flagged, not silently left in place.
- Final report lists both issues found and how each was resolved.

## Failure Conditions

- Only the new ADR is checked; the stale index entry is never surfaced.
- The index entry is hand-written without following `adr-capture`'s schema.
- The `.bak` file is treated as if it satisfies the missing ADR.
- No report distinguishing "added" from "removed" entries.
