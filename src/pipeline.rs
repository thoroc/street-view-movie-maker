//! Wires the modules together into the canonical pipeline (see the plan's
//! "Canonical download flow"): resolve/resume -> probe -> dedupe + turn frames
//! -> confirm/dry-run gate -> download -> lineup -> encode. This orchestration
//! layer isn't unit tested piece by piece — the plan reserves that for the
//! end-to-end and interrupt-resume tests in tests/pipeline.rs, since it's
//! mostly gluing already-tested modules together with real I/O.

use crate::pricing::{STATIC_MAP_PRICE_PER_1000_USD, STREETVIEW_PRICE_PER_1000_USD};
use crate::{
    cli, compositing, directions, geo, itinerary, lineup, maps, pricing, streetview, video,
};

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

pub async fn run() -> Result<(), String> {
    let args = cli::resolve_args(cli::Args::parse())?;

    let streetview_key = std::env::var("STREETVIEW_API_KEY").ok();
    let directions_key = std::env::var("DIRECTIONS_API_KEY").ok();
    cli::validate_api_keys(streetview_key.as_deref(), directions_key.as_deref())?;
    let streetview_key = streetview_key.expect("validated above");
    let directions_key = directions_key.expect("validated above");

    // Validate --picsize before any Street View API calls are billed — a
    // malformed or odd-numbered value used to only surface deep inside the
    // final ffmpeg encode step; it's a required, even "WIDTHxHEIGHT" now.
    video::parse_picsize(&args.picsize).map_err(|e| e.to_string())?;

    let output_dir = cli::resolve_output_dir(&args.output_dir, &args.output);
    let paths = cli::output_paths(&output_dir, &args.output);
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

    let client = reqwest::Client::new();

    let (records, start_display, end_display, fingerprint) = resume_or_fetch_itinerary(
        &client,
        &directions_key,
        &args,
        &tuning,
        &avoid,
        &paths.itinerary_path,
    )
    .await?;

    let sv_params = streetview::StreetviewParams {
        picsize: args.picsize.clone(),
        fov: args.fov,
        pitch: args.pitch,
        radius: args.radius,
    };

    let processed = probe_missing_frames(
        &client,
        &streetview_key,
        &sv_params,
        args.concurrency,
        records,
        tuning.turn_threshold,
        &paths.itinerary_path,
        &fingerprint,
    )
    .await?;

    let should_proceed = report_estimate_and_gate(
        &start_display,
        &end_display,
        processed.len(),
        args.hide_map,
        args.dry_run,
        args.yes,
    )?;
    if !should_proceed {
        return Ok(());
    }

    // The one Static Maps request happens here — after the confirmation
    // gate, alongside the Street View downloads below, never under
    // --dry-run. An auth/permission failure (the common case: the Maps
    // Static API isn't enabled on the project behind DIRECTIONS_API_KEY)
    // fails soft rather than aborting the whole run, since most existing
    // users won't have that API enabled yet.
    let route_points: Vec<(f64, f64)> = processed.iter().map(|r| (r.lat, r.lon)).collect();
    let map_state = fetch_map_inset(
        &client,
        &directions_key,
        args.hide_map,
        &args.map_size,
        &output_dir,
        &route_points,
    )
    .await?;

    let (image_paths, all_points) = download_frames(
        &client,
        &streetview_key,
        &sv_params,
        args.concurrency,
        &paths.images_dir,
        processed,
        &paths.itinerary_path,
        fingerprint,
    )
    .await?;

    let final_frame_paths = lineup_and_composite(
        image_paths,
        all_points,
        paths.lineup_dir.clone(),
        paths.composited_dir.clone(),
        map_state.as_ref(),
        &args.map_corner,
        args.concurrency,
    )
    .await?;

    encode_and_preview(
        final_frame_paths,
        &args.picsize,
        args.fps,
        &paths.video_path,
        &paths.preview_image_path,
    )
    .await
}

/// Resumes a persisted itinerary if its fingerprint matches, or fetches a
/// fresh route from the Directions API and builds a new itinerary otherwise.
async fn resume_or_fetch_itinerary(
    client: &reqwest::Client,
    directions_key: &str,
    args: &cli::ResolvedArgs,
    tuning: &itinerary::TuningParams,
    avoid: &[directions::AvoidFeature],
    itinerary_path: &std::path::Path,
) -> Result<(Vec<itinerary::PointRecord>, String, String, String), String> {
    let fingerprint = itinerary::route_fingerprint(&args.from, &args.to, tuning);
    let existing = itinerary::load_from(itinerary_path).map_err(|e| e.to_string())?;
    let decision = itinerary::resolve_resume(existing, &fingerprint, args.fresh);

    let (records, start_display, end_display) = match decision {
        itinerary::ResumeDecision::FingerprintMismatch { expected, found } => {
            return Err(format!(
                "persisted itinerary at {} was built for a different route/tuning (expected fingerprint {expected}, found {found}); pass --fresh to start over",
                itinerary_path.display()
            ));
        }
        itinerary::ResumeDecision::Resume(records) => (records, args.from.clone(), args.to.clone()),
        itinerary::ResumeDecision::Fresh => {
            let origin = directions::parse_route_endpoint(&args.from);
            let destination = directions::parse_route_endpoint(&args.to);
            let route =
                directions::fetch_directions(client, directions_key, &origin, &destination, avoid)
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

    Ok((records, start_display, end_display, fingerprint))
}

/// Probes Street View metadata for any record not already probed, then
/// dedupes by pano id and inserts turn frames — persisting the itinerary
/// after each stage that changes it.
// Each parameter is read independently; a bundling struct would obscure which
// ones this stage actually touches.
#[allow(clippy::too_many_arguments)]
async fn probe_missing_frames(
    client: &reqwest::Client,
    streetview_key: &str,
    sv_params: &streetview::StreetviewParams,
    concurrency: usize,
    mut records: Vec<itinerary::PointRecord>,
    turn_threshold: f64,
    itinerary_path: &std::path::Path,
    fingerprint: &str,
) -> Result<Vec<itinerary::PointRecord>, String> {
    let to_probe: Vec<(usize, f64, f64, f64)> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.status.is_none())
        .map(|(i, r)| (i, r.lat, r.lon, r.heading))
        .collect();
    if !to_probe.is_empty() {
        let probe_results = streetview::run_bounded(to_probe, concurrency, {
            let client = client.clone();
            let key = streetview_key.to_string();
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
            itinerary_path,
            &itinerary::ItineraryFile {
                fingerprint: fingerprint.to_string(),
                records: records.clone(),
            },
        )
        .map_err(|e| e.to_string())?;
    }

    let deduped = itinerary::dedupe_by_pano_id(&records);
    let processed = itinerary::insert_turn_frames(&deduped, turn_threshold);
    itinerary::save_to(
        itinerary_path,
        &itinerary::ItineraryFile {
            fingerprint: fingerprint.to_string(),
            records: processed.clone(),
        },
    )
    .map_err(|e| e.to_string())?;

    Ok(processed)
}

/// Prints the route summary and cost estimate, then applies the `--dry-run`
/// and confirm-to-download gates. Returns whether the pipeline should
/// continue past this point.
fn report_estimate_and_gate(
    start_display: &str,
    end_display: &str,
    image_count: usize,
    hide_map: bool,
    dry_run: bool,
    yes: bool,
) -> Result<bool, String> {
    println!("Route: {start_display} -> {end_display}");
    println!("Images to download: {image_count}");
    println!(
        "Estimated cost: up to ${:.2} (Street View Static API, ${STREETVIEW_PRICE_PER_1000_USD:.2}/1000 images — Google's first 10,000 images/month are free, so this may cost $0 if you're within that allowance this month; the CLI can't check your remaining quota). Metadata probing above was free.",
        pricing::estimate_download_cost_usd(image_count)
    );
    if !hide_map {
        println!(
            "Inset map: 1 additional Static Maps API call this run, ${:.4} (${STATIC_MAP_PRICE_PER_1000_USD:.2}/1000 requests — again, may be $0 within the free tier).",
            STATIC_MAP_PRICE_PER_1000_USD / 1000.0
        );
    }

    if dry_run {
        println!(
            "--dry-run set: stopping before download (the Directions call above was still billed)."
        );
        return Ok(false);
    }

    if !yes {
        println!(
            "Would you like to download them all? Type yes to proceed; otherwise, program halts."
        );
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| e.to_string())?;
        if !matches!(input.trim(), "yes" | "Yes") {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Fetches (or loads a cached) Static Maps inset image for the route, unless
/// `--hide-map` is set. Soft-fails to `None` if the Maps Static API isn't
/// enabled on the project behind `DIRECTIONS_API_KEY`; hard-fails on any
/// other error.
async fn fetch_map_inset(
    client: &reqwest::Client,
    directions_key: &str,
    hide_map: bool,
    map_size: &str,
    output_dir: &std::path::Path,
    route_points: &[(f64, f64)],
) -> Result<Option<MapState>, String> {
    if hide_map {
        return Ok(None);
    }

    let requested_crop = maps::parse_size(map_size).map_err(|e| e.to_string())?;
    let crop_size = (
        requested_crop.0.min(maps::BASE_MAP_SIZE.0),
        requested_crop.1.min(maps::BASE_MAP_SIZE.1),
    );
    let (center, zoom) =
        geo::bbox_center_zoom(route_points, maps::BASE_MAP_SIZE.0, maps::BASE_MAP_SIZE.1);
    // Base map size is fixed (see maps::BASE_MAP_SIZE), so the cached
    // image's content depends only on the route, not on --map-size —
    // no need to invalidate the cache when --map-size changes.
    let cache_path = output_dir.join("map.png");
    let request = maps::StaticMapRequest {
        size: maps::BASE_MAP_SIZE,
        zoom,
        center,
        points: route_points,
        color: maps::DEFAULT_PATH_COLOR,
        weight: maps::DEFAULT_PATH_WEIGHT,
    };
    match maps::fetch_or_load_map(client, directions_key, &cache_path, &request).await {
        Ok(path) => Ok(Some(MapState {
            path,
            center,
            zoom,
            size: maps::BASE_MAP_SIZE,
            crop_size,
        })),
        Err(maps::MapsError::Auth(_)) => {
            eprintln!(
                "map inset needs the Maps Static API enabled for the project behind DIRECTIONS_API_KEY — see https://console.cloud.google.com/apis/library/static-maps-backend.googleapis.com; continuing without the map inset"
            );
            Ok(None)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Downloads any not-yet-downloaded frame (bounded concurrency), persists the
/// itinerary with `downloaded` flags updated, and returns the frame paths
/// alongside each frame's route point (for later frame-to-point association).
// Each parameter is read independently; a bundling struct would obscure which
// ones this stage actually touches.
#[allow(clippy::too_many_arguments)]
async fn download_frames(
    client: &reqwest::Client,
    streetview_key: &str,
    sv_params: &streetview::StreetviewParams,
    concurrency: usize,
    images_dir: &std::path::Path,
    mut processed: Vec<itinerary::PointRecord>,
    itinerary_path: &std::path::Path,
    fingerprint: String,
) -> Result<(Vec<std::path::PathBuf>, Vec<(f64, f64, f64)>), String> {
    let download_inputs: Vec<(usize, f64, f64, f64, bool)> = processed
        .iter()
        .enumerate()
        .map(|(i, r)| (i, r.lat, r.lon, r.heading, r.downloaded))
        .collect();
    let download_results = streetview::run_bounded(download_inputs, concurrency, {
        let client = client.clone();
        let key = streetview_key.to_string();
        let params = sv_params.clone();
        let images_dir = images_dir.to_path_buf();
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
        itinerary_path,
        &itinerary::ItineraryFile {
            fingerprint,
            records: processed,
        },
    )
    .map_err(|e| e.to_string())?;

    Ok((image_paths, all_points))
}

/// Drops consecutive visually-identical frames, renumbers what's left
/// sequentially, and composites the map inset onto each frame if one was
/// fetched.
async fn lineup_and_composite(
    image_paths: Vec<std::path::PathBuf>,
    all_points: Vec<(f64, f64, f64)>,
    lineup_dir: std::path::PathBuf,
    composited_dir: std::path::PathBuf,
    map_state: Option<&MapState>,
    map_corner: &str,
    concurrency: usize,
) -> Result<Vec<std::path::PathBuf>, String> {
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

    let final_frame_paths = if let Some(map_state) = map_state {
        let corner = compositing::MapCorner::parse(map_corner)?;
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
        compositing::composite_all(
            frames,
            map_state.path.clone(),
            composited_dir,
            composite_params,
            concurrency,
        )
        .await?
    } else {
        frame_paths
    };

    Ok(final_frame_paths)
}

/// Encodes the final frames to video and writes a representative preview
/// still image (the middle frame) alongside it.
async fn encode_and_preview(
    final_frame_paths: Vec<std::path::PathBuf>,
    picsize: &str,
    fps: u32,
    video_path: &std::path::Path,
    preview_image_path: &std::path::Path,
) -> Result<(), String> {
    video::encode_video(final_frame_paths.clone(), picsize, fps, video_path)
        .await
        .map_err(|e| e.to_string())?;
    println!("Video written to {}", video_path.display());

    // A representative still (the middle frame of the final output, after
    // any map compositing) saved alongside the video under the same name,
    // for a quick preview without opening the video itself.
    if let Some(preview_source) = final_frame_paths.get(final_frame_paths.len() / 2) {
        std::fs::copy(preview_source, preview_image_path).map_err(|e| e.to_string())?;
        println!("Preview image written to {}", preview_image_path.display());
    }

    Ok(())
}
