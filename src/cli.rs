//! CLI flag schema, resolution (flags -> interactive prompts -> defaults), and
//! output-path layout.

use crate::prompt::{
    prompt_bool_with_default, prompt_choice_with_default, prompt_optional, prompt_parsed,
    prompt_required, prompt_with_default,
};
use usage::Cli;

/// Turn a route into a Street View movie
#[derive(Cli, Debug)]
#[usage(bin = "svmm", version = "0.1.0")]
pub struct Args {
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

    /// Show a road-sign-style overlay 2-3 seconds before a real navigation
    /// turn, sourced from Directions API maneuver data (not the unrelated
    /// --turn-threshold camera-smoothing heuristic)
    #[usage(long)]
    show_turn_signs: bool,

    /// Seconds of lead time before a turn the road-sign overlay appears
    #[usage(long)]
    turn_sign_lead_seconds: Option<f64>,

    /// Comma-separated frame numbers to drop before compositing/encoding
    /// (as shown in images/frameN.jpg), e.g. a pano that's facing the wrong
    /// way despite being the nearest match Street View has. Merged into this
    /// run's persisted exclusion list — once excluded, a frame number stays
    /// excluded on later resumes without needing to pass this again.
    #[usage(long)]
    exclude_frames: Option<String>,
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
const DEFAULT_TURN_SIGN_LEAD_SECONDS: f64 = 2.5;
const MAP_CORNER_CHOICES: [&str; 4] = ["top-left", "top-right", "bottom-left", "bottom-right"];

/// Args with every field settled: either passed on the command line, filled in
/// interactively, or defaulted. This is what the rest of `run()` operates on.
pub struct ResolvedArgs {
    pub from: String,
    pub to: String,
    pub output: String,
    pub output_dir: Option<String>,
    pub picsize: String,
    pub fov: u32,
    pub pitch: i32,
    pub radius: u32,
    pub hop_size: u32,
    pub turn_threshold: u32,
    pub fps: u32,
    pub yes: bool,
    pub concurrency: usize,
    pub dry_run: bool,
    pub fresh: bool,
    pub avoid_tolls: bool,
    pub avoid_highways: bool,
    pub avoid_ferries: bool,
    pub hide_map: bool,
    pub map_corner: String,
    pub map_size: String,
    pub show_turn_signs: bool,
    pub turn_sign_lead_seconds: f64,
    pub exclude_frames: Vec<usize>,
}

/// Parses `--exclude-frames`'s comma-separated value into frame numbers,
/// e.g. `"547,672,2462"` -> `[547, 672, 2462]`. An empty/absent value yields
/// an empty list; a non-numeric entry is a hard error rather than a silent
/// skip, since a typo'd frame number here silently keeps a known-bad frame
/// in the video.
fn parse_exclude_frames(raw: &Option<String>) -> Result<Vec<usize>, String> {
    match raw {
        None => Ok(Vec::new()),
        Some(s) if s.trim().is_empty() => Ok(Vec::new()),
        Some(s) => s
            .split(',')
            .map(|part| {
                part.trim()
                    .parse::<usize>()
                    .map_err(|_| format!("--exclude-frames: not a frame number: {part:?}"))
            })
            .collect(),
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
pub fn resolve_args(raw: Args) -> Result<ResolvedArgs, String> {
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
    let exclude_frames = parse_exclude_frames(&raw.exclude_frames)?;
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
        show_turn_signs: raw.show_turn_signs,
        turn_sign_lead_seconds: raw
            .turn_sign_lead_seconds
            .unwrap_or(DEFAULT_TURN_SIGN_LEAD_SECONDS),
        exclude_frames,
    })
}

fn resolve_args_interactive(raw: Args) -> Result<ResolvedArgs, String> {
    let exclude_frames = parse_exclude_frames(&raw.exclude_frames)?;
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
        show_turn_signs: if raw.show_turn_signs {
            true
        } else {
            prompt_bool_with_default("Show turn-ahead road-sign overlay", false)?
        },
        turn_sign_lead_seconds: match raw.turn_sign_lead_seconds {
            Some(v) => v,
            None => prompt_parsed(
                "Turn-ahead sign lead time, in seconds",
                DEFAULT_TURN_SIGN_LEAD_SECONDS,
            )?,
        },
        from,
        to,
        output,
        output_dir,
        yes: raw.yes,
        dry_run: raw.dry_run,
        fresh: raw.fresh,
        exclude_frames,
    })
}

pub fn resolve_output_dir(output_dir: &Option<String>, name: &str) -> std::path::PathBuf {
    match output_dir {
        Some(dir) => std::path::PathBuf::from(dir),
        None => std::path::PathBuf::from(format!("./output/{name}")),
    }
}

pub struct OutputPaths {
    pub images_dir: std::path::PathBuf,
    pub itinerary_path: std::path::PathBuf,
    pub lineup_dir: std::path::PathBuf,
    pub composited_dir: std::path::PathBuf,
    pub video_path: std::path::PathBuf,
    pub preview_image_path: std::path::PathBuf,
}

/// See the plan's "Output layout": everything for one run lives under a
/// single `--output-dir` root, constructed once here and passed into the
/// other modules rather than each inventing its own path scheme.
pub fn output_paths(dir: &std::path::Path, name: &str) -> OutputPaths {
    OutputPaths {
        images_dir: dir.join("images"),
        itinerary_path: dir.join("itinerary.json"),
        lineup_dir: dir.join("lineup"),
        composited_dir: dir.join("composited"),
        video_path: dir.join(format!("{name}.mp4")),
        preview_image_path: dir.join(format!("{name}.jpg")),
    }
}

pub fn validate_api_keys(streetview: Option<&str>, directions: Option<&str>) -> Result<(), String> {
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
}
