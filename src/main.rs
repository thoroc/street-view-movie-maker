mod cli;
mod compositing;
mod directions;
mod geo;
mod itinerary;
mod lineup;
mod maps;
mod net;
mod pricing;
mod prompt;
mod streetview;
mod video;

use pricing::{STATIC_MAP_PRICE_PER_1000_USD, STREETVIEW_PRICE_PER_1000_USD};

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
        pricing::estimate_download_cost_usd(processed.len())
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
