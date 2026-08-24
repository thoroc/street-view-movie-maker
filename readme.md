# Google Street View Movie Maker

It makes movies out of Google Street View images!

You provide point A and point B. It uses the Google Roads API to get directions from A to B, then repeatedly looks for Street View images along that path, and converts them into a movie.

## Rust CLI (`svmm`)

The tool has been ported to a Rust CLI (`svmm`), which is the only maintained way to run it. The original Python 2 implementation has been removed.

### Setup and usage

Full instructions — getting Google API credentials, storing them with [fnox](https://fnox.jdx.dev/) instead of a plaintext `.env`, running the CLI, and understanding what a run costs before you commit to it — are in **[docs/GETTING_STARTED.md](docs/GETTING_STARTED.md)**.

Quick start once `mise install` has run (pins the Rust toolchain, FFmpeg, and secrets tooling, and wires up [hk](https://hk.jdx.dev/) git hooks) and your keys are in fnox:

```sh
fnox exec -- cargo run --release -- --from "45.517146,-73.579837" --to "43.676533,-79.357132" --output my_route
```

Every run prints the resolved route, image count, and an estimated cost, then waits for confirmation (or exits under `--dry-run`) before downloading anything.

Prebuilt macOS/Linux/Windows binaries are published on the [Releases page](https://github.com/thoroc/street-view-movie-maker/releases) — see [docs/GETTING_STARTED.md § 6](docs/GETTING_STARTED.md#6-releasing) for what they do and don't include (notably: you still need `ffmpeg` on `PATH` yourself).

### Development

- `cargo test` runs the unit and integration test suite. Two integration tests that hit the real, billed Google APIs are marked `#[ignore]` — run them explicitly with `cargo test -- --ignored` once your API keys are set.
- `hk run check` / `hk fix` run the same formatting and lint checks as the git hooks, on demand.

## Project history

When using Google Maps to plan a route I haven't driven or walked before, I always though it would be nifty to be able to preview the directions as a video. Obviously, you can check out the route by looking at Street View at random points, or navigating in Street View mode itself. But these options are tedious!

Other people have had the same idea; at least two have created web services to do it:

- [Streetview Player](http://brianfolts.com/driver/)
- [Route View](http://routeview.org/VirtualRide/)

In each case, the user gives points A and B, the site fetches directions, picks a set of waypoints about 150m apart, and shows you the image at each waypoint. But the sites don't download the images and compile them into a movie; they just repeatedly reload Google Street View at each waypoint. I wanted to obtain a simulated video preview.

Also, both services are basically defunct now that using the Street View API requires billing. (I last checked the functional sites in 2017.)

## Project hurdles

Step 1 was downloading an image. I started with [the Street View API documentation](https://developers.google.com/maps/documentation/streetview/intro), but to get it working in Python, I used [this blog post](https://andrewpwheeler.wordpress.com/2015/12/28/using-python-to-grab-google-street-view-imagery/) as a reference.

Step 2 was computing the route and selecting GPS points along that route. Using the Directions API was straightforward with the [Python Client for Google Maps Services](https://github.com/googlemaps/google-maps-services-python).

Step 3 was computing the correct heading (compass direction) from A to B, which is really tricky! I ultimately found [someone else's code](https://gist.github.com/jeromer/2005586) to do this.

But there were lots of failed hacks in between. The math for computing distances and angles on spheres is very cool---the [Haversine formula](https://en.wikipedia.org/wiki/Haversine_formula), but I probably would have enjoyed learning about it more in high school. The best among them used the GeoPy package, but was ugly: with [GeoPy](https://geopy.readthedocs.io/), I couldn't compute the heading from A to B. But, given point A, a bearing, and a distance, I could compute a destination C. I could also compute the distance between any two points. So, I computed 360 potential destinations, each a degree apart and a fixed distance from A, and then found the one that was nearest to C, which gave the approximate heading.

Step 4 was concatenating the images into a movie, for which [FFMPEG](https://ffmpeg.org/) is indispensible!
