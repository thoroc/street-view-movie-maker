# Worked Examples and Troubleshooting

## Worked examples, one per verdict

Real entries from this skill's initial backfill (`.context/plans/2026-08-25-add-repo-scouting-log-skill.md`'s
Backfill data section), one per `verdict` value, showing the level of detail a good entry needs.

### `NOTHING_NEW`

```json
{"url":"https://github.com/tjhorner/streetlens","date":"2026-08-25","repo_name":"tjhorner/streetlens","summary":"Self-hosted web app (NestJS/SvelteKit) for browsing your own recorded GoPro Max 360 footage as street-level imagery; extracts stills from video, the inverse of svmm.","verdict":"NOTHING_NEW","reasoning":"Different domain entirely; no provider, no adjacency graph, no video-encoding insight."}
```

Note `reasoning` still explains *why* it's a null result, not just that it is one -- "different domain" alone
wouldn't tell a future reader whether the domain difference was actually checked or just assumed.

### `INFORMS_EXISTING`

```json
{"url":"https://github.com/sk-zk/streetlevel","date":"2026-08-25","repo_name":"sk-zk/streetlevel","summary":"Python library fetching panorama images/metadata across many street-level providers via unofficial internal APIs; models Google's own linked/nearby-pano adjacency graph.","verdict":"INFORMS_EXISTING","reasoning":"Added a 6th, structurally different candidate (pano-adjacency-graph heading) to the turn-detector known-issue, and a regional-providers section to the Google-alternatives finding.","related":[".context/known-issues/2026-08-25-geometric-turn-detector-fires-on-gentle-curves.md",".context/findings/2026-08-25-inset-map-provider-cost-alternatives-to-google-static-maps.md"]}
```

Two `related` paths because this one investigation informed two separate open findings -- `related` is a list for
exactly this reason, not a single optional pointer.

### `NEW_FINDING_NEEDED`

```json
{"url":"https://github.com/proog128/HyperlapseMB","date":"2026-08-25","repo_name":"proog128/HyperlapseMB","summary":"A Hyperlapse.js fork; adds a WebGL depth-aware per-pixel motion-blur shader using Street View's own depth metadata; no frame selection of its own.","verdict":"NEW_FINDING_NEEDED","reasoning":"A genuinely new, real technique, but orthogonal to both tracked gaps and doesn't fit svmm's current architecture. Flagged to the user as a candidate for its own standalone finding; not yet filed as of this entry."}
```

No `related` yet -- honest about the fact that nothing has been filed for this one, rather than inventing a
placeholder path. Update the entry (or add a new one) once a finding actually exists.

## Troubleshooting

```text
Problem                                          | Solution
Validation fails: a field doesn't match          | validate-repo-scouting-log.sh caught a malformed field --
                                                  | fix it to match repo-scouting-entry.schema.json's constraints.
Validation fails: duplicate url                  | Another entry already covers this repo -- read its verdict/
                                                  | reasoning instead of re-investigating from scratch.
Validation fails: schema required fields         | The script's known field list is out of sync with a schema
mismatch                                         | edit -- update validate-repo-scouting-log.sh's expected_fields.
Validation warns: related path doesn't exist     | Non-fatal by design -- the referenced finding/plan may not be
                                                  | written yet. Fix the path or file the referenced document.
Log file doesn't exist yet                       | Not an error -- this is the first entry. Create
                                                  | .context/repo-scouting/log.jsonl and the parent directory.
Entry logged in a worktree isn't visible          | .context/ is untracked, so each worktree has its own copy --
elsewhere                                        | sync log.jsonl back to the main checkout (Rule 4).
```
