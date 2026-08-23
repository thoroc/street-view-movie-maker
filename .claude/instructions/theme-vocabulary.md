# Theme Vocabulary and Tagging Rules

The `themes` frontmatter field records **what area of the system** a `.context/` action-candidate is about. It is the
subject axis, orthogonal to the magnitude axes (`value`, `effort`, `severity`): those say how big or urgent an item is,
`themes` says what it touches.

`themes` applies to the three action-candidate types: `PLAN`, `FINDING`, and `KNOWN_ISSUE`. It does not apply to
`ANALYSIS`, `INSTRUCTION`, or `AUDIT`, which are reference material rather than things to do next.

## Shape

`themes` is a **multi-valued, ordered list** — an entry can genuinely belong to several areas, so it is not a single
enum. Every member is drawn from the controlled vocabulary below; free-form text is not permitted, so the axis stays
queryable.

```yaml
themes:
  - ROUTING
  - CLI
```

The list is **ordered, and the first entry (`themes[0]`) is the primary theme.** The primary answers "what is this
mainly about?" and is the only member that participates in the read-protocol tie-break (see below). The remaining
members are for filtering and cluster views, never for ordering. Authors write the list primary-first.

## Controlled vocabulary

Six themes, kept deliberately coarse, mapped to `svmm`'s actual module layout (see `docs/GETTING_STARTED.md` and the
port plan at `.context/plans/2026-08-22-port-street-view-movie-maker-to-rust-cli.md`).

| Theme        | Covers                                                                                                                    |
| ------------ | -------------------------------------------------------------------------------------------------------------------------- |
| `ROUTING`    | Directions API integration, polyline decoding, geo math (`directions.rs`, `geo.rs`), route fingerprinting and resume, avoid-options (tolls/highways/ferries). |
| `STREETVIEW` | Street View metadata probing and image download, pano dedupe (`streetview.rs`, `itinerary.rs`).                          |
| `VIDEO`      | Frame lineup/renumbering and ffmpeg encoding (`lineup.rs`, `video.rs`), overlays (inset map, turn signs).                |
| `CLI`        | Argument parsing, interactive prompts, confirmation/cost-estimate gates, `--dry-run`/`--fresh` (`main.rs`).              |
| `GOVERNANCE` | The `.context/` system and project docs: the index, frontmatter contract, ADR capture, and cross-reference integrity.    |
| `TOOLING`    | Developer workflow: the mise toolchain, pre-commit hooks, the `.claude/` skills/hooks themselves, and CI.                |

## Split-on-evidence rule

The vocabulary ships coarse and is refined only on observed need, not speculatively. If, after backfill, a single theme
carries a disproportionate share of entries (rough guide: more than ~30% of active/draft action-candidates), split it
into finer themes. Record any split as a decision in `docs/adr/` if it's consequential enough to warrant one (see
ADR-worthiness in `planning-flow.md`); otherwise a short `.context/` note documenting the change is enough.

## Choosing the primary theme — worked examples

The primary is the area the item most changes, not merely one it touches.

- **A plan to add `--avoid-tolls`/`--avoid-highways`/`--avoid-ferries` → `[ROUTING, CLI]`.** Primarily a Directions API
  change, with a CLI-surface component.
- **A finding about pano dedupe missing near-duplicate frames → `[STREETVIEW]`.** Purely about the probe/download
  layer.
- **A plan to add an inset map overlay to the rendered video → `[VIDEO, ROUTING]`.** Primarily a rendering change that
  also needs the route geometry.
- **A plan to change the `.context/` frontmatter contract or the ADR index → `[GOVERNANCE]`.** It changes the
  governance system itself.
- **A plan to add a pre-commit check that also touches CLI arg validation → `[TOOLING, CLI]`.** Primarily a
  workflow/tooling change (`TOOLING`), but it touches CLI code, so `CLI` is a genuine secondary — the primary is still
  the tooling mechanism.

## Read protocol interaction

`themes[0]` is the final tie-breaker in the "what's next" sort, below `value` then `effort`:

1. Filter to `DRAFT`/`ACTIVE` `PLAN`/`FINDING`/`KNOWN_ISSUE`.
2. Sort by `value` descending (`HIGH` > `MEDIUM` > `LOW`).
3. Then `effort` ascending (`S` < `M` < `L` < `TBD`) where present.
4. Then, only to break a remaining tie, prefer the item whose `themes[0]` matches the area already in focus. Theme
   expresses preference-of-area, not priority, which is why it sits below both magnitude axes.

`themes` is also a filter/slice dimension in its own right: "show me all `ROUTING` work" or "which theme carries the
most open debt" are queries the index answers by reading the field directly, independent of the sort. See
[`value-rubric.md`](value-rubric.md) for the full read protocol.
