# Rust style and maintainability

## Reference material

Two curated resources are the default lookup for idiomatic-Rust questions before improvising a style:

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) — naming, trait impls, error types, and other
  public-interface conventions. Consult this before adding or changing a `pub` item.
- [rust-unofficial/patterns](https://rust-unofficial.github.io/patterns/) — idiom catalogue (newtype, builder, typestate,
  RAII guards). Check here before hand-rolling a pattern one of these already names.

`clippy` already enforces a large share of what these two documents describe; it runs at `pre-push` per `hk.pkl`. These
two are for the judgment calls clippy can't lint — API shape and pattern choice, not syntax.

## Maintainability, specific to this codebase

- **`src/main.rs` is the decomposition hotspot.** It's 654 lines, with a 223-line `run()` and an 86-line `resolve_args()`.
  When touching it, extract a cohesive step into a named function or a new module rather than adding to `run()` — don't
  let it keep growing. `itinerary.rs` (417 lines) and `streetview.rs` (410 lines) are next largest; apply the same
  discipline before adding new responsibilities to either.
- **One error enum per module boundary, `thiserror`-style.** `directions.rs::DirectionsError` is the existing pattern —
  match it for new modules instead of returning `String` or `anyhow::Error` across a public boundary. Add context at the
  point of failure (`map_err` with a variant), not by re-wrapping generic errors upstream.
- **Keep codec detail behind the encoder boundary.** Per [ADR-013](../../docs/ADR/adr-013-native-rust-codec-pure-rust-default-opt-in-h264.md),
  `video/av1` and `video/h264` internals (OBU parsing, `rav1e` config, muxing) must not leak into `main.rs` or
  `lineup.rs` — those should depend on the encoder's public interface, not its codec-specific types.
- **Prefer newtypes over raw primitives for domain identifiers.** A route ID, pano ID, or frame index passed as a bare
  `String`/`u64` can be silently swapped with another of the same primitive type at a call site; a newtype makes that a
  compile error. Reach for this when a function's signature has more than one same-typed identifier parameter.
