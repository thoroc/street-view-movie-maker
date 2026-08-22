//! Integration tests for the assembled pipeline (see the plan's Phase 6).
//!
//! Most of these need real, billed Google API credentials and are marked
//! `#[ignore]` — run them explicitly with `cargo test -- --ignored` once
//! STREETVIEW_API_KEY/DIRECTIONS_API_KEY are set to real values. The
//! fingerprint-mismatch test needs neither network access nor real keys,
//! since it's rejected before any API call happens, so it runs by default.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_output_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("svmm_test_pipeline_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_binary(args: &[&str], api_keys: (&str, &str)) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_svmm"))
        .args(args)
        .env("STREETVIEW_API_KEY", api_keys.0)
        .env("DIRECTIONS_API_KEY", api_keys.1)
        .output()
        .expect("failed to run svmm binary")
}

#[test]
fn resuming_a_different_route_under_the_same_output_name_is_rejected() {
    let dir = temp_output_dir();
    let itinerary_path = dir.join("itinerary.json");
    std::fs::write(
        &itinerary_path,
        r#"{"fingerprint":"deadbeef-not-the-real-fingerprint","records":[]}"#,
    )
    .unwrap();

    let output = run_binary(
        &[
            "--from",
            "1.0,1.0",
            "--to",
            "2.0,2.0",
            "--output",
            "test",
            "--output-dir",
            dir.to_str().unwrap(),
            "--yes",
        ],
        ("dummy-streetview-key", "dummy-directions-key"),
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("different route"),
        "expected a route-mismatch error, got: {stderr}"
    );
    assert!(!dir.join("test.mp4").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "hits the real, billed Directions/Street View APIs — run with `cargo test -- --ignored` and real API keys set"]
fn end_to_end_short_real_route_produces_a_video() {
    let streetview_key =
        std::env::var("STREETVIEW_API_KEY").expect("set STREETVIEW_API_KEY to run this test");
    let directions_key =
        std::env::var("DIRECTIONS_API_KEY").expect("set DIRECTIONS_API_KEY to run this test");
    let dir = temp_output_dir();

    let output = run_binary(
        &[
            "--from",
            "33.669793,-115.802125",
            "--to",
            "33.671796,-115.801851",
            "--output",
            "e2e_test",
            "--output-dir",
            dir.to_str().unwrap(),
            "--yes",
        ],
        (&streetview_key, &directions_key),
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.join("e2e_test.mp4").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "hits the real, billed Directions/Street View APIs — run with `cargo test -- --ignored` and real API keys set"]
fn interrupted_run_resumes_without_redownloading() {
    let streetview_key =
        std::env::var("STREETVIEW_API_KEY").expect("set STREETVIEW_API_KEY to run this test");
    let directions_key =
        std::env::var("DIRECTIONS_API_KEY").expect("set DIRECTIONS_API_KEY to run this test");
    let dir = temp_output_dir();
    let args = [
        "--from",
        "33.669793,-115.802125",
        "--to",
        "33.671796,-115.801851",
        "--output",
        "resume_test",
        "--output-dir",
        dir.to_str().unwrap(),
        "--yes",
        "--dry-run",
    ];

    // First run: probes and persists itinerary state, but downloads nothing
    // (--dry-run), simulating an interruption before the download phase.
    let first = run_binary(&args, (&streetview_key, &directions_key));
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let itinerary_path = dir.join("itinerary.json");
    assert!(itinerary_path.exists());
    let after_first_probe = std::fs::read_to_string(&itinerary_path).unwrap();

    // Second run without --dry-run: should resume the persisted itinerary
    // (the default behavior) rather than re-probing/re-resolving the route.
    let resumed_args: Vec<&str> = args
        .iter()
        .filter(|a| **a != "--dry-run")
        .copied()
        .collect();
    let second = run_binary(&resumed_args, (&streetview_key, &directions_key));
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(dir.join("resume_test.mp4").exists());

    let after_second = std::fs::read_to_string(&itinerary_path).unwrap();
    assert!(
        after_second.contains(&after_first_probe[..50.min(after_first_probe.len())]),
        "expected the resumed itinerary to build on the first run's probed data"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
