mod compositing;
mod directions;
mod geo;
mod itinerary;
mod lineup;
mod maps;
mod net;
mod streetview;
mod video;

use usage::Cli;

/// Turn a route into a Street View movie
#[derive(Cli, Debug)]
#[usage(bin = "svmm", version = "0.1.0")]
struct Args {
    /// Origin: "lat,lon" or a free-text place/address
    #[usage(long)]
    from: Option<String>,

    /// Destination: "lat,lon" or a free-text place/address
    #[usage(long)]
    to: Option<String>,

    /// Output filestem / video name (default: "<from>-<to>-<datetime>")
    #[usage(long)]
    output: Option<String>,

    /// Root directory for this run's images, itinerary state, and video
    #[usage(long)]
    output_dir: Option<String>,

    /// Downloaded Street View image size
    #[usage(long)]
    picsize: Option<String>,

    /// Street View camera field of view, in degrees
    #[usage(long)]
    fov: Option<u32>,

    /// Street View camera pitch, in degrees
    #[usage(long)]
    pitch: Option<i32>,

    /// Street View search radius, in meters
    #[usage(long)]
    radius: Option<u32>,

    /// Distance between interpolated route points, in meters
    #[usage(long)]
    hop_size: Option<u32>,

    /// Heading delta, in degrees, that triggers a turn frame insertion
    #[usage(long)]
    turn_threshold: Option<u32>,

    /// Output video frame rate
    #[usage(long)]
    fps: Option<u32>,

    /// Skip the download confirmation prompt
    #[usage(long)]
    yes: bool,

    /// Max concurrent Street View requests
    #[usage(long)]
    concurrency: Option<usize>,

    /// Probe and print the final image count, then exit without downloading
    #[usage(long)]
    dry_run: bool,

    /// Ignore persisted itinerary state and start over
    #[usage(long)]
    fresh: bool,

    /// Prompt for any value not already given as a flag, showing each default as a suggestion
    #[usage(long, short)]
    interactive: bool,

    /// Avoid tolls when routing (matches Google Maps' "Avoid tolls" toggle)
    #[usage(long)]
    avoid_tolls: bool,

    /// Avoid highways when routing (matches Google Maps' "Avoid highways" toggle)
    #[usage(long)]
    avoid_highways: bool,

    /// Avoid ferries when routing (matches Google Maps' "Avoid ferries" toggle)
    #[usage(long)]
    avoid_ferries: bool,

    /// Hide the inset route map (shown in a frame corner by default)
    #[usage(long)]
    hide_map: bool,

    /// Corner of the frame the inset route map is placed in
    #[usage(long, choices("top-left", "top-right", "bottom-left", "bottom-right"))]
    map_corner: Option<String>,

    /// Size of the local-area window panned around the current position,
    /// e.g. "200x200" (cropped from a larger fixed-resolution base map so
    /// the inset stays centered on you as the video progresses); the
    /// on-frame footprint is a separate, percentage-based size, not
    /// controlled by this flag
    #[usage(long)]
    map_size: Option<String>,
}

/// Values used when a field is left unset outside of --interactive.
const DEFAULT_PICSIZE: &str = "640x320";
const DEFAULT_FOV: u32 = 90;
const DEFAULT_PITCH: i32 = 0;
const DEFAULT_RADIUS: u32 = 5;
const DEFAULT_HOP_SIZE: u32 = 10;
const DEFAULT_TURN_THRESHOLD: u32 = 5;
const DEFAULT_FPS: u32 = 20;
const DEFAULT_CONCURRENCY: usize = 5;
const DEFAULT_MAP_CORNER: &str = "bottom-right";
const DEFAULT_MAP_SIZE: &str = "200x200";
const MAP_CORNER_CHOICES: [&str; 4] = ["top-left", "top-right", "bottom-left", "bottom-right"];

/// Args with every field settled: either passed on the command line, filled in
/// interactively, or defaulted. This is what the rest of `run()` operates on.
struct ResolvedArgs {
    from: String,
    to: String,
    output: String,
    output_dir: Option<String>,
    picsize: String,
    fov: u32,
    pitch: i32,
    radius: u32,
    hop_size: u32,
    turn_threshold: u32,
    fps: u32,
    yes: bool,
    concurrency: usize,
    dry_run: bool,
    fresh: bool,
    avoid_tolls: bool,
    avoid_highways: bool,
    avoid_ferries: bool,
    hide_map: bool,
    map_corner: String,
    map_size: String,
}

fn prompt_line(label: &str) -> Result<String, String> {
    print!("{label}: ");
    std::io::Write::flush(&mut std::io::stdout()).map_err(|e| e.to_string())?;
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| e.to_string())?;
    Ok(input.trim().to_string())
}

fn prompt_required(label: &str) -> Result<String, String> {
    loop {
        let value = prompt_line(&format!("{label} (required)"))?;
        if !value.is_empty() {
            return Ok(value);
        }
        println!("  a value is required.");
    }
}

fn prompt_optional(label: &str) -> Result<Option<String>, String> {
    let value = prompt_line(label)?;
    Ok(if value.is_empty() { None } else { Some(value) })
}

fn prompt_with_default(label: &str, default: &str) -> Result<String, String> {
    let value = prompt_line(&format!("{label} [{default}]"))?;
    Ok(if value.is_empty() {
        default.to_string()
    } else {
        value
    })
}

fn prompt_bool_with_default(label: &str, default: bool) -> Result<bool, String> {
    let hint = if default { "Y/n" } else { "y/N" };
    loop {
        let raw = prompt_line(&format!("{label} [{hint}]"))?;
        match raw.trim().to_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("  please answer y or n."),
        }
    }
}

fn prompt_choice_with_default(
    label: &str,
    default: &str,
    choices: &[&str],
) -> Result<String, String> {
    loop {
        let raw = prompt_line(&format!("{label} [{default}] ({})", choices.join("/")))?;
        if raw.is_empty() {
            return Ok(default.to_string());
        }
        if choices.contains(&raw.as_str()) {
            return Ok(raw);
        }
        println!("  please choose one of: {}", choices.join(", "));
    }
}

fn prompt_parsed<T>(label: &str, default: T) -> Result<T, String>
where
    T: std::fmt::Display + std::str::FromStr,
    T::Err: std::fmt::Display,
{
    loop {
        let raw = prompt_line(&format!("{label} [{default}]"))?;
        if raw.is_empty() {
            return Ok(default);
        }
        match raw.parse::<T>() {
            Ok(value) => return Ok(value),
            Err(e) => println!("  invalid value: {e}. try again."),
        }
    }
}

fn missing_flag_error(flag: &str) -> String {
    format!(
        "the following required argument was not provided: {flag}\n\ntip: pass --interactive/-i to be prompted for it instead"
    )
}

/// Lowercases `input` and collapses every run of non-alphanumeric characters
/// into a single `-`, trimming leading/trailing dashes — turns a `--from`/
/// `--to` value ("lat,lon" or a free-text place) into a filesystem-safe
/// name segment.
fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Default `--output` value when not given explicitly: "<from>-<to>-<datetime>".
fn default_output_name(from: &str, to: &str) -> String {
    format!(
        "{}-{}-{}",
        slugify(from),
        slugify(to),
        chrono::Local::now().format("%Y%m%dT%H%M%S")
    )
}

/// Fills in whatever `--interactive` didn't already get from flags, using each
/// field's default as the suggested value; outside of `--interactive`, applies
/// those same defaults directly and errors if a required field is still missing.
fn resolve_args(raw: Args) -> Result<ResolvedArgs, String> {
    if raw.interactive {
        resolve_args_interactive(raw)
    } else {
        resolve_args_noninteractive(raw)
    }
}

fn resolve_args_noninteractive(raw: Args) -> Result<ResolvedArgs, String> {
    let from = raw.from.ok_or_else(|| missing_flag_error("--from"))?;
    let to = raw.to.ok_or_else(|| missing_flag_error("--to"))?;
    let output = raw
        .output
        .unwrap_or_else(|| default_output_name(&from, &to));
    Ok(ResolvedArgs {
        from,
        to,
        output,
        output_dir: raw.output_dir,
        picsize: raw.picsize.unwrap_or_else(|| DEFAULT_PICSIZE.to_string()),
        fov: raw.fov.unwrap_or(DEFAULT_FOV),
        pitch: raw.pitch.unwrap_or(DEFAULT_PITCH),
        radius: raw.radius.unwrap_or(DEFAULT_RADIUS),
        hop_size: raw.hop_size.unwrap_or(DEFAULT_HOP_SIZE),
        turn_threshold: raw.turn_threshold.unwrap_or(DEFAULT_TURN_THRESHOLD),
        fps: raw.fps.unwrap_or(DEFAULT_FPS),
        concurrency: raw.concurrency.unwrap_or(DEFAULT_CONCURRENCY),
        yes: raw.yes,
        dry_run: raw.dry_run,
        fresh: raw.fresh,
        avoid_tolls: raw.avoid_tolls,
        avoid_highways: raw.avoid_highways,
        avoid_ferries: raw.avoid_ferries,
        hide_map: raw.hide_map,
        map_corner: raw
            .map_corner
            .unwrap_or_else(|| DEFAULT_MAP_CORNER.to_string()),
        map_size: raw.map_size.unwrap_or_else(|| DEFAULT_MAP_SIZE.to_string()),
    })
}

fn resolve_args_interactive(raw: Args) -> Result<ResolvedArgs, String> {
    let from = match raw.from {
        Some(v) => v,
        None => prompt_required("Origin (\"lat,lon\" or a place/address)")?,
    };
    let to = match raw.to {
        Some(v) => v,
        None => prompt_required("Destination (\"lat,lon\" or a place/address)")?,
    };
    let output = match raw.output {
        Some(v) => v,
        None => prompt_with_default(
            "Output filestem / video name",
            &default_output_name(&from, &to),
        )?,
    };
    let output_dir = match raw.output_dir {
        Some(v) => Some(v),
        None => prompt_optional(&format!("Output directory (blank for ./output/{output})"))?,
    };

    Ok(ResolvedArgs {
        picsize: match raw.picsize {
            Some(v) => v,
            None => prompt_with_default("Downloaded Street View image size", DEFAULT_PICSIZE)?,
        },
        fov: match raw.fov {
            Some(v) => v,
            None => prompt_parsed("Street View camera field of view, in degrees", DEFAULT_FOV)?,
        },
        pitch: match raw.pitch {
            Some(v) => v,
            None => prompt_parsed("Street View camera pitch, in degrees", DEFAULT_PITCH)?,
        },
        radius: match raw.radius {
            Some(v) => v,
            None => prompt_parsed("Street View search radius, in meters", DEFAULT_RADIUS)?,
        },
        hop_size: match raw.hop_size {
            Some(v) => v,
            None => prompt_parsed(
                "Distance between interpolated route points, in meters",
                DEFAULT_HOP_SIZE,
            )?,
        },
        turn_threshold: match raw.turn_threshold {
            Some(v) => v,
            None => prompt_parsed(
                "Heading delta, in degrees, that triggers a turn frame insertion",
                DEFAULT_TURN_THRESHOLD,
            )?,
        },
        fps: match raw.fps {
            Some(v) => v,
            None => prompt_parsed("Output video frame rate", DEFAULT_FPS)?,
        },
        concurrency: match raw.concurrency {
            Some(v) => v,
            None => prompt_parsed("Max concurrent Street View requests", DEFAULT_CONCURRENCY)?,
        },
        avoid_tolls: if raw.avoid_tolls {
            true
        } else {
            prompt_bool_with_default("Avoid tolls", false)?
        },
        avoid_highways: if raw.avoid_highways {
            true
        } else {
            prompt_bool_with_default("Avoid highways", false)?
        },
        avoid_ferries: if raw.avoid_ferries {
            true
        } else {
            prompt_bool_with_default("Avoid ferries", false)?
        },
        map_size: match raw.map_size {
            Some(v) => v,
            None => prompt_with_default(
                "Inset map's local-area window size, panned around your position (e.g. 200x200)",
                DEFAULT_MAP_SIZE,
            )?,
        },
        map_corner: match raw.map_corner {
            Some(v) => v,
            None => prompt_choice_with_default(
                "Inset map corner",
                DEFAULT_MAP_CORNER,
                &MAP_CORNER_CHOICES,
            )?,
        },
        hide_map: if raw.hide_map {
            true
        } else {
            prompt_bool_with_default("Hide inset map", false)?
        },
        from,
        to,
        output,
        output_dir,
        yes: raw.yes,
        dry_run: raw.dry_run,
        fresh: raw.fresh,
    })
}

fn resolve_output_dir(output_dir: &Option<String>, name: &str) -> std::path::PathBuf {
    match output_dir {
        Some(dir) => std::path::PathBuf::from(dir),
        None => std::path::PathBuf::from(format!("./output/{name}")),
    }
}

struct OutputPaths {
    images_dir: std::path::PathBuf,
    itinerary_path: std::path::PathBuf,
    lineup_dir: std::path::PathBuf,
    composited_dir: std::path::PathBuf,
    video_path: std::path::PathBuf,
    preview_image_path: std::path::PathBuf,
}

/// See the plan's "Output layout": everything for one run lives under a
/// single `--output-dir` root, constructed once here and passed into the
/// other modules rather than each inventing its own path scheme.
fn output_paths(dir: &std::path::Path, name: &str) -> OutputPaths {
    OutputPaths {
        images_dir: dir.join("images"),
        itinerary_path: dir.join("itinerary.json"),
        lineup_dir: dir.join("lineup"),
        composited_dir: dir.join("composited"),
        video_path: dir.join(format!("{name}.mp4")),
        preview_image_path: dir.join(format!("{name}.jpg")),
    }
}

/// Street View Static API pricing per Google's published rate card
/// (developers.google.com/maps/billing-and-pricing/pricing, last checked
/// 2026-08-23): $7.00 per 1,000 images, with the first 10,000/month free.
/// This estimate assumes no free-tier allowance remains this month, since
/// the CLI has no way to know how much of it you've already used — the
/// actual charge may be $0 if you're within that allowance.
const STREETVIEW_PRICE_PER_1000_USD: f64 = 7.00;

/// Maps Static API pricing per Google's published rate card (same source as
/// the Street View constant above, last checked 2026-08-23): $2.00 per 1,000
/// requests, with the first 10,000/month free. Only one Static Maps request
/// happens per run regardless of frame count.
const STATIC_MAP_PRICE_PER_1000_USD: f64 = 2.00;

fn estimate_download_cost_usd(image_count: usize) -> f64 {
    image_count as f64 / 1000.0 * STREETVIEW_PRICE_PER_1000_USD
}

/// The one Static Maps image fetched per run, plus what's needed to place
/// the per-frame marker on it (see `geo::lat_lon_to_pixel`) and pan/crop a
/// local-area window around it (see `compositing::crop_window`).
struct MapState {
    path: std::path::PathBuf,
    center: (f64, f64),
    zoom: u32,
    size: (u32, u32),
    crop_size: (u32, u32),
}

fn validate_api_keys(streetview: Option<&str>, directions: Option<&str>) -> Result<(), String> {
    let missing: Vec<&str> = [
        ("STREETVIEW_API_KEY", streetview),
        ("DIRECTIONS_API_KEY", directions),
    ]
    .into_iter()
    .filter(|(_, value)| value.is_none_or(str::is_empty))
    .map(|(name, _)| name)
    .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "missing required environment variable(s): {}",
            missing.join(", ")
        ))
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

/// Wires the modules together into the canonical pipeline (see the plan's
/// "Canonical download flow"): resolve/resume → probe → dedupe + turn frames
/// → confirm/dry-run gate → download → lineup → encode. This orchestration
/// layer isn't unit tested piece by piece — the plan reserves that for the
/// end-to-end and interrupt-resume tests in tests/pipeline.rs, since it's
/// mostly gluing already-tested modules together with real I/O.
async fn run() -> Result<(), String> {
    let args = resolve_args(Args::parse())?;

    let streetview_key = std::env::var("STREETVIEW_API_KEY").ok();
    let directions_key = std::env::var("DIRECTIONS_API_KEY").ok();
    validate_api_keys(streetview_key.as_deref(), directions_key.as_deref())?;
    let streetview_key = streetview_key.expect("validated above");
    let directions_key = directions_key.expect("validated above");

    // Validate --picsize before any Street View API calls are billed — a
    // malformed or odd-numbered value used to only surface deep inside the
    // final ffmpeg encode step; it's a required, even "WIDTHxHEIGHT" now.
    video::parse_picsize(&args.picsize).map_err(|e| e.to_string())?;

    let output_dir = resolve_output_dir(&args.output_dir, &args.output);
    let paths = output_paths(&output_dir, &args.output);
    std::fs::create_dir_all(&paths.images_dir).map_err(|e| e.to_string())?;

    let tuning = itinerary::TuningParams {
        hop_size: f64::from(args.hop_size),
        turn_threshold: f64::from(args.turn_threshold),
        picsize: args.picsize.clone(),
        fov: args.fov,
        pitch: args.pitch,
        radius: args.radius,
        avoid_tolls: args.avoid_tolls,
        avoid_highways: args.avoid_highways,
        avoid_ferries: args.avoid_ferries,
    };
    let avoid: Vec<directions::AvoidFeature> = [
        (args.avoid_tolls, directions::AvoidFeature::Tolls),
        (args.avoid_highways, directions::AvoidFeature::Highways),
        (args.avoid_ferries, directions::AvoidFeature::Ferries),
    ]
    .into_iter()
    .filter_map(|(enabled, feature)| enabled.then_some(feature))
    .collect();
    let fingerprint = itinerary::route_fingerprint(&args.from, &args.to, &tuning);
    let existing = itinerary::load_from(&paths.itinerary_path).map_err(|e| e.to_string())?;
    let decision = itinerary::resolve_resume(existing, &fingerprint, args.fresh);

    let client = reqwest::Client::new();

    let (mut records, start_display, end_display) = match decision {
        itinerary::ResumeDecision::FingerprintMismatch { expected, found } => {
            return Err(format!(
                "persisted itinerary at {} was built for a different route/tuning (expected fingerprint {expected}, found {found}); pass --fresh to start over",
                paths.itinerary_path.display()
            ));
        }
        itinerary::ResumeDecision::Resume(records) => (records, args.from.clone(), args.to.clone()),
        itinerary::ResumeDecision::Fresh => {
            let origin = directions::parse_route_endpoint(&args.from);
            let destination = directions::parse_route_endpoint(&args.to);
            let route = directions::fetch_directions(
                &client,
                &directions_key,
                &origin,
                &destination,
                &avoid,
            )
            .await
            .map_err(|e| e.to_string())?;

            let mut points: Vec<(f64, f64)> = Vec::new();
            if route.points.len() < 2 {
                points.push(route.start);
            } else {
                for pair in route.points.windows(2) {
                    points.extend(geo::interpolate_points_by_hop(
                        pair[0],
                        pair[1],
                        tuning.hop_size,
                    ));
                }
            }
            let points = geo::clean_look_points(&points);
            let records = itinerary::build_itinerary(&points);

            let start_display = route
                .start_address
                .clone()
                .unwrap_or_else(|| format!("{:?}", route.start));
            let end_display = route
                .end_address
                .clone()
                .unwrap_or_else(|| format!("{:?}", route.end));
            (records, start_display, end_display)
        }
    };

    let sv_params = streetview::StreetviewParams {
        picsize: args.picsize.clone(),
        fov: args.fov,
        pitch: args.pitch,
        radius: args.radius,
    };

    let to_probe: Vec<(usize, f64, f64, f64)> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.status.is_none())
        .map(|(i, r)| (i, r.lat, r.lon, r.heading))
        .collect();
    if !to_probe.is_empty() {
        let probe_results = streetview::run_bounded(to_probe, args.concurrency, {
            let client = client.clone();
            let key = streetview_key.clone();
            let params = sv_params.clone();
            move |(i, lat, lon, heading)| {
                let client = client.clone();
                let key = key.clone();
                let params = params.clone();
                async move {
                    let result =
                        streetview::probe_metadata(&client, &key, lat, lon, heading, &params).await;
                    (i, result)
                }
            }
        })
        .await;
        for (i, result) in probe_results {
            let meta = result.map_err(|e| format!("probing Street View metadata failed: {e}"))?;
            records[i].status = Some(meta.status);
            records[i].pano_id = meta.pano_id;
            records[i].copyright = meta.copyright;
        }
        itinerary::save_to(
            &paths.itinerary_path,
            &itinerary::ItineraryFile {
                fingerprint: fingerprint.clone(),
                records: records.clone(),
            },
        )
        .map_err(|e| e.to_string())?;
    }

    let deduped = itinerary::dedupe_by_pano_id(&records);
    let mut processed = itinerary::insert_turn_frames(&deduped, tuning.turn_threshold);
    itinerary::save_to(
        &paths.itinerary_path,
        &itinerary::ItineraryFile {
            fingerprint: fingerprint.clone(),
            records: processed.clone(),
        },
    )
    .map_err(|e| e.to_string())?;

    println!("Route: {start_display} -> {end_display}");
    println!("Images to download: {}", processed.len());
    println!(
        "Estimated cost: up to ${:.2} (Street View Static API, ${STREETVIEW_PRICE_PER_1000_USD:.2}/1000 images — Google's first 10,000 images/month are free, so this may cost $0 if you're within that allowance this month; the CLI can't check your remaining quota). Metadata probing above was free.",
        estimate_download_cost_usd(processed.len())
    );
    if !args.hide_map {
        println!(
            "Inset map: 1 additional Static Maps API call this run, ${:.4} (${STATIC_MAP_PRICE_PER_1000_USD:.2}/1000 requests — again, may be $0 within the free tier).",
            STATIC_MAP_PRICE_PER_1000_USD / 1000.0
        );
    }

    if args.dry_run {
        println!(
            "--dry-run set: stopping before download (the Directions call above was still billed)."
        );
        return Ok(());
    }

    if !args.yes {
        println!(
            "Would you like to download them all? Type yes to proceed; otherwise, program halts."
        );
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| e.to_string())?;
        if !matches!(input.trim(), "yes" | "Yes") {
            return Ok(());
        }
    }

    // The one Static Maps request happens here — after the confirmation
    // gate, alongside the Street View downloads below, never under
    // --dry-run. An auth/permission failure (the common case: the Maps
    // Static API isn't enabled on the project behind DIRECTIONS_API_KEY)
    // fails soft rather than aborting the whole run, since most existing
    // users won't have that API enabled yet.
    let map_state: Option<MapState> = if args.hide_map {
        None
    } else {
        let requested_crop = maps::parse_size(&args.map_size).map_err(|e| e.to_string())?;
        let crop_size = (
            requested_crop.0.min(maps::BASE_MAP_SIZE.0),
            requested_crop.1.min(maps::BASE_MAP_SIZE.1),
        );
        let route_points: Vec<(f64, f64)> = processed.iter().map(|r| (r.lat, r.lon)).collect();
        let (center, zoom) =
            geo::bbox_center_zoom(&route_points, maps::BASE_MAP_SIZE.0, maps::BASE_MAP_SIZE.1);
        // Base map size is fixed (see maps::BASE_MAP_SIZE), so the cached
        // image's content depends only on the route, not on --map-size —
        // no need to invalidate the cache when --map-size changes.
        let cache_path = output_dir.join("map.png");
        let request = maps::StaticMapRequest {
            size: maps::BASE_MAP_SIZE,
            zoom,
            center,
            points: &route_points,
            color: maps::DEFAULT_PATH_COLOR,
            weight: maps::DEFAULT_PATH_WEIGHT,
        };
        match maps::fetch_or_load_map(&client, &directions_key, &cache_path, &request).await {
            Ok(path) => Some(MapState {
                path,
                center,
                zoom,
                size: maps::BASE_MAP_SIZE,
                crop_size,
            }),
            Err(maps::MapsError::Auth(_)) => {
                eprintln!(
                    "map inset needs the Maps Static API enabled for the project behind DIRECTIONS_API_KEY — see https://console.cloud.google.com/apis/library/static-maps-backend.googleapis.com; continuing without the map inset"
                );
                None
            }
            Err(e) => return Err(e.to_string()),
        }
    };

    let download_inputs: Vec<(usize, f64, f64, f64, bool)> = processed
        .iter()
        .enumerate()
        .map(|(i, r)| (i, r.lat, r.lon, r.heading, r.downloaded))
        .collect();
    let download_results = streetview::run_bounded(download_inputs, args.concurrency, {
        let client = client.clone();
        let key = streetview_key.clone();
        let params = sv_params.clone();
        let images_dir = paths.images_dir.clone();
        move |(i, lat, lon, heading, downloaded)| {
            let client = client.clone();
            let key = key.clone();
            let params = params.clone();
            let dest = images_dir.join(format!("frame{i}.jpg"));
            async move {
                if downloaded {
                    return (i, Ok(dest));
                }
                let result =
                    streetview::download_image(&client, &key, lat, lon, heading, &params, &dest)
                        .await;
                (i, result.map(|()| dest))
            }
        }
    })
    .await;

    let mut image_paths: Vec<std::path::PathBuf> = Vec::with_capacity(processed.len());
    for (i, result) in download_results {
        let path = result.map_err(|e| format!("downloading image failed: {e}"))?;
        processed[i].downloaded = true;
        image_paths.push(path);
    }
    let all_points: Vec<(f64, f64, f64)> = processed
        .iter()
        .map(|r| (r.lat, r.lon, r.heading))
        .collect();
    itinerary::save_to(
        &paths.itinerary_path,
        &itinerary::ItineraryFile {
            fingerprint,
            records: processed,
        },
    )
    .map_err(|e| e.to_string())?;

    // `lineup::dedupe_by_content` drops consecutive visually-identical
    // frames, which shifts frame-number vs. route-point correspondence — so
    // it's the *indices* it kept, not just the renumbered paths, that let
    // the compositing pass below know which route point each surviving
    // frame came from (see the plan's "frame-to-point association" note).
    let lineup_dir = paths.lineup_dir.clone();
    let (frame_paths, frame_points) = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let kept = lineup::dedupe_by_content_indices(&image_paths).map_err(|e| e.to_string())?;
        let kept_paths: Vec<std::path::PathBuf> =
            kept.iter().map(|&i| image_paths[i].clone()).collect();
        let renumbered = lineup::renumber_sequentially(&kept_paths, &lineup_dir, "frame")
            .map_err(|e| e.to_string())?;
        let kept_points: Vec<(f64, f64, f64)> = kept.iter().map(|&i| all_points[i]).collect();
        Ok((renumbered, kept_points))
    })
    .await
    .map_err(|e| e.to_string())??;

    let (_, final_frame_paths) = if let Some(map_state) = &map_state {
        let corner = compositing::MapCorner::parse(&args.map_corner)?;
        let composite_params = compositing::CompositeParams {
            corner,
            margin_percent: compositing::INSET_MARGIN_PERCENT,
            footprint_percent: compositing::INSET_FOOTPRINT_PERCENT,
            map_center: map_state.center,
            map_zoom: map_state.zoom,
            map_size: map_state.size,
            crop_size: map_state.crop_size,
        };
        let frames: Vec<(usize, std::path::PathBuf, (f64, f64, f64))> = frame_paths
            .into_iter()
            .zip(frame_points)
            .enumerate()
            .map(|(i, (path, point))| (i, path, point))
            .collect();
        let composited_paths = compositing::composite_all(
            frames,
            map_state.path.clone(),
            paths.composited_dir.clone(),
            composite_params,
            args.concurrency,
        )
        .await?;
        (paths.composited_dir.clone(), composited_paths)
    } else {
        (paths.lineup_dir.clone(), frame_paths)
    };

    video::encode_video(
        final_frame_paths.clone(),
        &args.picsize,
        args.fps,
        &paths.video_path,
    )
    .await
    .map_err(|e| e.to_string())?;
    println!("Video written to {}", paths.video_path.display());

    // A representative still (the middle frame of the final output, after
    // any map compositing) saved alongside the video under the same name,
    // for a quick preview without opening the video itself.
    if let Some(preview_source) = final_frame_paths.get(final_frame_paths.len() / 2) {
        std::fs::copy(preview_source, &paths.preview_image_path).map_err(|e| e.to_string())?;
        println!(
            "Preview image written to {}",
            paths.preview_image_path.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Args;
    use usage::test as harness;

    #[test]
    fn malformed_concurrency_flag_fails_parsing() {
        let words = harness::argv([
            "--from",
            "0.0,0.0",
            "--to",
            "1.0,1.0",
            "--output",
            "test",
            "--concurrency",
            "not-a-number",
        ]);
        let message = harness::parse(Args::spec(), &words.words(), Args::parse_from).unwrap_err();
        assert!(
            message.to_string().contains("invalid value"),
            "expected an 'invalid value' diagnostic, got: {message}"
        );
    }

    #[test]
    fn negative_hop_size_flag_fails_parsing() {
        let words = harness::argv([
            "--from",
            "0.0,0.0",
            "--to",
            "1.0,1.0",
            "--output",
            "test",
            "--hop-size=-5",
        ]);
        let message = harness::parse(Args::spec(), &words.words(), Args::parse_from).unwrap_err();
        assert!(
            message.to_string().contains("invalid value"),
            "expected an 'invalid value' diagnostic, got: {message}"
        );
    }

    #[test]
    fn validate_api_keys_reports_all_missing_keys() {
        let err = super::validate_api_keys(None, None).unwrap_err();
        assert!(err.contains("STREETVIEW_API_KEY"));
        assert!(err.contains("DIRECTIONS_API_KEY"));
    }

    #[test]
    fn validate_api_keys_rejects_empty_string_key() {
        let err = super::validate_api_keys(Some(""), Some("b")).unwrap_err();
        assert!(err.contains("STREETVIEW_API_KEY"));
        assert!(!err.contains("DIRECTIONS_API_KEY"));
    }

    #[test]
    fn validate_api_keys_passes_when_both_present() {
        assert!(super::validate_api_keys(Some("a"), Some("b")).is_ok());
    }

    #[test]
    fn resolve_output_dir_defaults_to_output_slash_name() {
        let dir = super::resolve_output_dir(&None, "joshua_tree");
        assert_eq!(dir, std::path::PathBuf::from("./output/joshua_tree"));
    }

    #[test]
    fn resolve_output_dir_uses_the_given_path_when_set() {
        let dir = super::resolve_output_dir(&Some("/custom/dir".to_string()), "joshua_tree");
        assert_eq!(dir, std::path::PathBuf::from("/custom/dir"));
    }

    #[test]
    fn output_paths_matches_the_plans_layout() {
        let dir = std::path::Path::new("./output/joshua_tree");
        let paths = super::output_paths(dir, "joshua_tree");
        assert_eq!(paths.images_dir, dir.join("images"));
        assert_eq!(paths.itinerary_path, dir.join("itinerary.json"));
        assert_eq!(paths.lineup_dir, dir.join("lineup"));
        assert_eq!(paths.composited_dir, dir.join("composited"));
        assert_eq!(paths.video_path, dir.join("joshua_tree.mp4"));
        assert_eq!(paths.preview_image_path, dir.join("joshua_tree.jpg"));
    }

    #[test]
    fn slugify_lowercases_and_collapses_punctuation() {
        assert_eq!(super::slugify("48.8611,2.3358"), "48-8611-2-3358");
        assert_eq!(
            super::slugify("Marseille Provence Airport"),
            "marseille-provence-airport"
        );
    }

    #[test]
    fn slugify_trims_leading_and_trailing_dashes() {
        assert_eq!(
            super::slugify("  -leading and trailing- "),
            "leading-and-trailing"
        );
    }

    #[test]
    fn default_output_name_includes_both_slugified_endpoints() {
        let name = super::default_output_name("48.8611,2.3358", "Simiane-la-Rotonde");
        assert!(name.starts_with("48-8611-2-3358-simiane-la-rotonde-"));
    }

    #[test]
    fn estimate_download_cost_usd_scales_with_image_count() {
        assert_eq!(super::estimate_download_cost_usd(0), 0.0);
        assert_eq!(super::estimate_download_cost_usd(1000), 7.0);
        assert_eq!(super::estimate_download_cost_usd(500), 3.5);
    }

    #[test]
    fn estimate_download_cost_usd_matches_a_realistic_route_size() {
        // A ~90km route (like the Marseille-airport-to-Simiane-la-Rotonde
        // example) produced ~8500 images in practice.
        let cost = super::estimate_download_cost_usd(8500);
        assert!(
            (cost - 59.5).abs() < 1e-9,
            "expected ~$59.50 for 8500 images, got ${cost}"
        );
    }
}
