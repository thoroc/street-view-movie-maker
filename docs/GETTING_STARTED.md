# Getting started with `svmm`

This covers getting Google API credentials, storing them safely, running the CLI, and understanding what a run costs before you commit to it.

## 1. Get Google API credentials

The CLI calls two Google Maps Platform APIs directly, and relies on a third indirectly:

- **Directions API** — turns `--from`/`--to` into a route.
- **Street View Static API** — downloads the actual frames.
- **Geocoding API** (indirect) — when `--from`/`--to` is a place name rather than `"lat,lon"`, the Directions API resolves it internally using Geocoding. You don't call it yourself, but it must be enabled on your project or place-name resolution fails.

Setup:

1. Go to the [Google Cloud Console](https://console.cloud.google.com/) and create or select a project.
2. In **APIs & Services → Library**, enable **Directions API**, **Street View Static API**, and **Geocoding API**.
3. **Enable billing** on the project — Street View Static API requires it even though usage is often free at low volume (see [Cost](#4-cost) below).
4. In **APIs & Services → Credentials**, create an API key. One key works across all three APIs. Optionally restrict it (API restrictions to just those three, and/or application restrictions) since it'll sit in your local secrets store.
5. Official docs, if you want the detail: [Street View API key setup](https://developers.google.com/maps/documentation/streetview/get-api-key), [Directions API key setup](https://developers.google.com/maps/documentation/directions/get-api-key).

## 2. Store the keys with fnox

Don't put real keys in a plaintext `.env`. This repo's `fnox.toml` is configured with an `age` encryption provider, so only ciphertext ever gets committed:

```sh
# one-time: generate your local age key if you don't already have one
mkdir -p ~/.config/fnox && age-keygen -o ~/.config/fnox/age.txt

# store both keys (prompts interactively — never on the command line)
fnox set STREETVIEW_API_KEY --provider age
fnox set DIRECTIONS_API_KEY --provider age
```

Run the CLI with `fnox exec -- ...` so the keys are loaded as env vars for that command only, or `eval "$(fnox activate bash)"` (or `zsh`/`fish`) to auto-load them whenever you `cd` into this repo.

## 3. Run it

```sh
fnox exec -- cargo run --release -- --from "Marseille Provence Airport" --to "Simiane-la-Rotonde" --output my_trip
```

`--from`/`--to` accept `"lat,lon"` or a free-text place name — both go through the same resolution path. Run `cargo run -- --help` for the full flag list. One gotcha: pass negative numeric values with `=`, e.g. `--pitch=-30`, not `--pitch -30` — a bare `-30` after a space is read as an unrecognized flag, not this flag's value.

Every run prints the resolved route, the number of images it will download, and an estimated cost, then stops for confirmation (or exits immediately with `--dry-run`) before anything is downloaded. Nothing is downloaded without that gate unless you pass `--yes`.

### Prefer prompts to flags? Use `--interactive`

Don't want to memorise flag names, or unsure what `--fov`, `--radius`, or `--hop-size` should be? Pass `-i`/`--interactive` and the CLI asks for each value in turn, showing its default in brackets — press enter to accept it, or type a value to override:

```sh
fnox exec -- cargo run --release -- --interactive
```

`--interactive` fills in only what's missing. Combine it with any flags you already know:

```sh
fnox exec -- cargo run --release -- --interactive --from "Marseille Provence Airport" --to "Simiane-la-Rotonde"
```

Here `--from`/`--to` are used as given, and you're only prompted for `--output` and the remaining tuning flags. Without `--interactive`, any of `--from`/`--to`/`--output` left unset causes the CLI to error out and suggest `--interactive` instead of guessing a value for you.

### Steering the route: `--avoid-tolls`/`--avoid-highways`/`--avoid-ferries`

These mirror Google Maps' own "Avoid tolls/highways/ferries" toggles: pass any combination to steer the one route the Directions API returns away from that feature. All three default to off (avoid nothing), matching Maps' own default:

```sh
fnox exec -- cargo run --release -- --from "Marseille Provence Airport" --to "Simiane-la-Rotonde" --output my_trip --avoid-tolls --avoid-highways
```

Under `--interactive`, you're prompted for each (y/N) alongside the other tuning values.

## 4. Cost

Prices below are from Google's published rate card (developers.google.com/maps/billing-and-pricing/pricing, checked 2026-08-23) — verify against the live console before relying on them at volume, since Google's pricing changes.

| API call | Price | Monthly free allowance |
| --- | --- | --- |
| Street View **metadata** probe (checking if a panorama exists at a point) | Free, unlimited | n/a |
| Street View **Static** image (the actual downloaded frame) | $7.00 / 1,000 | First 10,000 |
| Directions (one call per run) | $5.00 / 1,000 | First 10,000 |

The CLI can't see how much of your monthly free allowance you've already used, so the cost line it prints is a **worst-case estimate assuming none remains** — the real charge may be $0. Use `--dry-run` to see the resolved route, image count, and cost estimate without downloading anything (note: the one Directions call still happens and is billed, negligibly, under `--dry-run` too).

### Image count doesn't shrink much with `--hop-size`

For a real, non-trivial route (tens of km), you might expect a larger `--hop-size` (the spacing between sampled points, default 10m) to proportionally cut the image count. In practice it doesn't, much: image count is mostly set by how many distinct Street View panoramas actually exist along the route (typically one every 10-20m on covered roads), and `--hop-size` just needs to be small enough not to skip past panoramas — increasing it well beyond that spacing eventually reduces count, but not until it's much larger than the natural panorama density.

Example measured on a real ~90km airport-to-village route:

| `--hop-size` | `--turn-threshold` | Images |
| --- | --- | --- |
| 50 (default 10) | default (5) | 9,198 |
| 100 | 20 | 7,831 |
| 200 | default | 8,641 |
| 1000 | default | 8,507 |

`--turn-threshold` (how much a heading has to change before extra turn frames get inserted) has more effect than `--hop-size` on a winding road, but even a generous threshold only trims the count by 10-20% here — most of it is genuine panorama density, not an artifact of the tuning flags. If you want a meaningfully shorter/cheaper video for a long route, `--hop-size` in the many-hundreds-of-meters range is the main lever available today; there's no built-in "keep every Nth frame" downsampling yet.

## 5. Resuming and starting over

Re-running the same `--output` name resumes from persisted state (`<output-dir>/itinerary.json`) rather than re-probing and re-downloading everything — useful if a run gets interrupted. If you change `--from`/`--to` or the tuning flags for the same `--output` name, the CLI detects the mismatch and refuses to resume (to avoid silently mixing two different routes); pass `--fresh` to start over deliberately.
