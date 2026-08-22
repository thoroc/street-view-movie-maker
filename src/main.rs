mod directions;
mod geo;
mod itinerary;
mod net;
mod streetview;

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

fn main() {
    let _args = Args::parse();
    let streetview_key = std::env::var("STREETVIEW_API_KEY").ok();
    let directions_key = std::env::var("DIRECTIONS_API_KEY").ok();
    if let Err(err) = validate_api_keys(streetview_key.as_deref(), directions_key.as_deref()) {
        eprintln!("{err}");
        std::process::exit(1);
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
}
