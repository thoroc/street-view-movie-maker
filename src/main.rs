mod directions;
mod geo;
mod itinerary;
mod lineup;
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
    from: String,

    /// Destination: "lat,lon" or a free-text place/address
    #[usage(long)]
    to: String,

    /// Output filestem / video name
    #[usage(long)]
    output: String,

    /// Root directory for this run's images, itinerary state, and video
    #[usage(long)]
    output_dir: Option<String>,

    /// Downloaded Street View image size
    #[usage(long, default = "640x320")]
    picsize: String,

    /// Street View camera field of view, in degrees
    #[usage(long, default = "90")]
    fov: u32,

    /// Street View camera pitch, in degrees
    #[usage(long, default = "0")]
    pitch: i32,

    /// Street View search radius, in meters
    #[usage(long, default = "5")]
    radius: u32,

    /// Distance between interpolated route points, in meters
    #[usage(long, default = "10")]
    hop_size: u32,

    /// Heading delta, in degrees, that triggers a turn frame insertion
    #[usage(long, default = "5")]
    turn_threshold: u32,

    /// Output video frame rate
    #[usage(long, default = "20")]
    fps: u32,

    /// Skip the download confirmation prompt
    #[usage(long)]
    yes: bool,

    /// Max concurrent Street View requests
    #[usage(long, default = "5")]
    concurrency: usize,

    /// Probe and print the final image count, then exit without downloading
    #[usage(long)]
    dry_run: bool,

    /// Ignore persisted itinerary state and start over
    #[usage(long)]
    fresh: bool,
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
    video_path: std::path::PathBuf,
}

/// See the plan's "Output layout": everything for one run lives under a
/// single `--output-dir` root, constructed once here and passed into the
/// other modules rather than each inventing its own path scheme.
fn output_paths(dir: &std::path::Path, name: &str) -> OutputPaths {
    OutputPaths {
        images_dir: dir.join("images"),
        itinerary_path: dir.join("itinerary.json"),
        lineup_dir: dir.join("lineup"),
        video_path: dir.join(format!("{name}.mp4")),
    }
}

/// Street View Static API pricing per Google's published rate card
/// (developers.google.com/maps/billing-and-pricing/pricing, last checked
/// 2026-08-23): $7.00 per 1,000 images, with the first 10,000/month free.
/// This estimate assumes no free-tier allowance remains this month, since
/// the CLI has no way to know how much of it you've already used — the
/// actual charge may be $0 if you're within that allowance.
const STREETVIEW_PRICE_PER_1000_USD: f64 = 7.00;

fn estimate_download_cost_usd(image_count: usize) -> f64 {
    image_count as f64 / 1000.0 * STREETVIEW_PRICE_PER_1000_USD
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
    let args = Args::parse();

    let streetview_key = std::env::var("STREETVIEW_API_KEY").ok();
    let directions_key = std::env::var("DIRECTIONS_API_KEY").ok();
    validate_api_keys(streetview_key.as_deref(), directions_key.as_deref())?;
    let streetview_key = streetview_key.expect("validated above");
    let directions_key = directions_key.expect("validated above");

    video::check_ffmpeg_available()
        .await
        .map_err(|e| e.to_string())?;

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
    };
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
            let route =
                directions::fetch_directions(&client, &directions_key, &origin, &destination)
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
    itinerary::save_to(
        &paths.itinerary_path,
        &itinerary::ItineraryFile {
            fingerprint,
            records: processed,
        },
    )
    .map_err(|e| e.to_string())?;

    let lineup_dir = paths.lineup_dir.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let deduped = lineup::dedupe_by_content(&image_paths).map_err(|e| e.to_string())?;
        lineup::renumber_sequentially(&deduped, &lineup_dir, "frame").map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    video::encode_video(
        &paths.lineup_dir,
        "frame",
        args.fps,
        &args.picsize,
        &paths.video_path,
    )
    .await
    .map_err(|e| e.to_string())?;

    println!("Video written to {}", paths.video_path.display());
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
        assert_eq!(paths.video_path, dir.join("joshua_tree.mp4"));
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
