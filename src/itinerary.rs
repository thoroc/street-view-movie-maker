use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PointRecord {
    pub lat: f64,
    pub lon: f64,
    pub heading: f64,
    pub pano_id: Option<String>,
    pub status: Option<String>,
    pub copyright: Option<String>,
    pub downloaded: bool,
}

/// Builds the initial itinerary from a point list, computing headings toward
/// the next point (the last point reuses the previous heading), before any
/// Street View probing has happened. Mirrors `create_itinerary_df`.
pub fn build_itinerary(points: &[(f64, f64)]) -> Vec<PointRecord> {
    let mut records: Vec<PointRecord> = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let heading = if i + 1 < points.len() {
            crate::geo::initial_compass_bearing(points[i], points[i + 1])
        } else if i > 0 {
            records[i - 1].heading
        } else {
            0.0
        };
        records.push(PointRecord {
            lat: points[i].0,
            lon: points[i].1,
            heading,
            pano_id: None,
            status: None,
            copyright: None,
            downloaded: false,
        });
    }
    records
}

/// Keeps only the first record for each unique pano_id, dropping records with
/// no pano_id, a non-"OK" status, or non-Google copyright. Mirrors
/// `process_pointlist`'s `unique_panos`/`keepers` filtering.
pub fn dedupe_by_pano_id(records: &[PointRecord]) -> Vec<PointRecord> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for record in records {
        let Some(pano_id) = record.pano_id.as_deref() else {
            continue;
        };
        if record.status.as_deref() != Some("OK") {
            continue;
        }
        if !crate::streetview::is_from_google(&record.copyright) {
            continue;
        }
        if seen.insert(pano_id) {
            out.push(record.clone());
        }
    }
    out
}

/// Inserts extra frames between consecutive records whose heading changes by
/// more than `threshold_deg`, at the earlier record's position, so turns
/// don't jump abruptly in the final video. Mirrors `process_pointlist`'s
/// turn-frame insertion (stepsize=1, matching the Python default).
pub fn insert_turn_frames(records: &[PointRecord], threshold_deg: f64) -> Vec<PointRecord> {
    let mut out = Vec::with_capacity(records.len());
    for i in 0..records.len() {
        out.push(records[i].clone());
        if i + 1 >= records.len() {
            continue;
        }
        let delta = (records[i + 1].heading - records[i].heading).abs();
        if delta <= threshold_deg {
            continue;
        }
        let headings = crate::geo::turn_headings(records[i].heading, records[i + 1].heading, 1.0);
        for &heading in headings
            .iter()
            .skip(1)
            .take(headings.len().saturating_sub(2))
        {
            out.push(PointRecord {
                heading,
                ..records[i].clone()
            });
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct TuningParams {
    pub hop_size: f64,
    pub turn_threshold: f64,
    pub picsize: String,
    pub fov: u32,
    pub pitch: i32,
    pub radius: u32,
    pub avoid_tolls: bool,
    pub avoid_highways: bool,
    pub avoid_ferries: bool,
}

/// Hashes the resolved route endpoints plus tuning flags into a short
/// fingerprint, stored alongside persisted itinerary state so a resume
/// attempt against a different route/tuning combination under the same
/// `--output` name is rejected instead of silently mixing data.
pub fn route_fingerprint(from: &str, to: &str, tuning: &TuningParams) -> String {
    let mut hasher = Sha256::new();
    hasher.update(from.as_bytes());
    hasher.update(b"|");
    hasher.update(to.as_bytes());
    hasher.update(b"|");
    hasher.update(tuning.hop_size.to_bits().to_le_bytes());
    hasher.update(tuning.turn_threshold.to_bits().to_le_bytes());
    hasher.update(tuning.picsize.as_bytes());
    hasher.update(tuning.fov.to_le_bytes());
    hasher.update(tuning.pitch.to_le_bytes());
    hasher.update(tuning.radius.to_le_bytes());
    hasher.update([
        tuning.avoid_tolls as u8,
        tuning.avoid_highways as u8,
        tuning.avoid_ferries as u8,
    ]);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItineraryFile {
    pub fingerprint: String,
    pub records: Vec<PointRecord>,
}

pub fn save_to(path: &std::path::Path, file: &ItineraryFile) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(file)?;
    std::fs::write(path, json)
}

pub fn load_from(path: &std::path::Path) -> std::io::Result<Option<ItineraryFile>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let file = serde_json::from_str(&contents)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Ok(Some(file))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[derive(Debug)]
pub enum ResumeDecision {
    Fresh,
    Resume(Vec<PointRecord>),
    FingerprintMismatch { expected: String, found: String },
}

/// Decides whether to resume from persisted itinerary state (see the plan's
/// itinerary.rs "route fingerprint" note). `--fresh` always wins; otherwise a
/// fingerprint mismatch is a hard error rather than a silent mixed-route
/// resume.
pub fn resolve_resume(
    existing: Option<ItineraryFile>,
    current_fingerprint: &str,
    fresh: bool,
) -> ResumeDecision {
    if fresh {
        return ResumeDecision::Fresh;
    }
    match existing {
        None => ResumeDecision::Fresh,
        Some(file) if file.fingerprint == current_fingerprint => {
            ResumeDecision::Resume(file.records)
        }
        Some(file) => ResumeDecision::FingerprintMismatch {
            expected: current_fingerprint.to_string(),
            found: file.fingerprint,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_path() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "svmm_test_itinerary_{}_{n}.json",
            std::process::id()
        ))
    }

    fn tuning() -> TuningParams {
        TuningParams {
            hop_size: 10.0,
            turn_threshold: 5.0,
            picsize: "640x320".to_string(),
            fov: 90,
            pitch: 0,
            radius: 5,
            avoid_tolls: false,
            avoid_highways: false,
            avoid_ferries: false,
        }
    }

    #[test]
    fn build_itinerary_computes_headings_between_consecutive_points() {
        let points = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
        let records = build_itinerary(&points);
        assert_eq!(records.len(), 3);
        assert_eq!(
            records[0].heading,
            crate::geo::initial_compass_bearing(points[0], points[1])
        );
        assert_eq!(
            records[1].heading,
            crate::geo::initial_compass_bearing(points[1], points[2])
        );
    }

    #[test]
    fn build_itinerary_last_point_reuses_previous_heading() {
        let points = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
        let records = build_itinerary(&points);
        assert_eq!(records[2].heading, records[1].heading);
    }

    #[test]
    fn build_itinerary_starts_unprobed_and_undownloaded() {
        let points = vec![(0.0, 0.0), (0.0, 1.0)];
        let records = build_itinerary(&points);
        for r in &records {
            assert_eq!(r.status, None);
            assert_eq!(r.pano_id, None);
            assert!(!r.downloaded);
        }
    }

    fn record(
        lat: f64,
        lon: f64,
        heading: f64,
        pano_id: &str,
        status: &str,
        copyright: &str,
    ) -> PointRecord {
        PointRecord {
            lat,
            lon,
            heading,
            pano_id: Some(pano_id.to_string()),
            status: Some(status.to_string()),
            copyright: Some(copyright.to_string()),
            downloaded: false,
        }
    }

    #[test]
    fn dedupe_keeps_first_occurrence_of_each_pano_id() {
        let records = vec![
            record(0.0, 0.0, 0.0, "A", "OK", "\u{a9} Google"),
            record(0.0, 0.1, 5.0, "A", "OK", "\u{a9} Google"),
            record(0.0, 0.2, 10.0, "B", "OK", "\u{a9} Google"),
        ];
        let deduped = dedupe_by_pano_id(&records);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].pano_id.as_deref(), Some("A"));
        assert_eq!(deduped[0].heading, 0.0);
        assert_eq!(deduped[1].pano_id.as_deref(), Some("B"));
    }

    #[test]
    fn dedupe_drops_non_ok_status() {
        let records = vec![
            record(0.0, 0.0, 0.0, "A", "ZERO_RESULTS", "\u{a9} Google"),
            record(0.0, 0.1, 5.0, "B", "OK", "\u{a9} Google"),
        ];
        let deduped = dedupe_by_pano_id(&records);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].pano_id.as_deref(), Some("B"));
    }

    #[test]
    fn dedupe_drops_non_google_copyright() {
        let records = vec![
            record(0.0, 0.0, 0.0, "A", "OK", "\u{a9} Jane Doe"),
            record(0.0, 0.1, 5.0, "B", "OK", "\u{a9} Google"),
        ];
        let deduped = dedupe_by_pano_id(&records);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].pano_id.as_deref(), Some("B"));
    }

    #[test]
    fn dedupe_drops_records_with_no_pano_id() {
        let mut unprobed = record(0.0, 0.0, 0.0, "A", "OK", "\u{a9} Google");
        unprobed.pano_id = None;
        let records = vec![unprobed, record(0.0, 0.1, 5.0, "B", "OK", "\u{a9} Google")];
        let deduped = dedupe_by_pano_id(&records);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].pano_id.as_deref(), Some("B"));
    }

    #[test]
    fn insert_turn_frames_leaves_small_heading_deltas_untouched() {
        let records = vec![
            record(0.0, 0.0, 10.0, "A", "OK", "\u{a9} Google"),
            record(0.0, 0.1, 12.0, "B", "OK", "\u{a9} Google"),
        ];
        let out = insert_turn_frames(&records, 5.0);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn insert_turn_frames_adds_frames_for_large_heading_deltas() {
        let records = vec![
            record(0.0, 0.0, 0.0, "A", "OK", "\u{a9} Google"),
            record(0.0, 0.1, 90.0, "B", "OK", "\u{a9} Google"),
        ];
        let out = insert_turn_frames(&records, 5.0);
        // get_turn_headings(0, 90, stepsize=1)[1:-1] inserts 88 intermediate frames.
        assert_eq!(out.len(), 2 + 88);
        // Inserted frames sit at the turning point's lat/lon.
        assert_eq!(out[1].lat, 0.0);
        assert_eq!(out[1].lon, 0.0);
        assert!(out[1].heading > 0.0 && out[1].heading < 90.0);
        assert_eq!(out.last().unwrap().pano_id.as_deref(), Some("B"));
    }

    #[test]
    fn route_fingerprint_is_deterministic() {
        let a = route_fingerprint("45.0,-73.0", "43.0,-79.0", &tuning());
        let b = route_fingerprint("45.0,-73.0", "43.0,-79.0", &tuning());
        assert_eq!(a, b);
    }

    #[test]
    fn route_fingerprint_differs_when_route_differs() {
        let a = route_fingerprint("45.0,-73.0", "43.0,-79.0", &tuning());
        let b = route_fingerprint("45.0,-73.0", "44.0,-79.0", &tuning());
        assert_ne!(a, b);
    }

    #[test]
    fn route_fingerprint_differs_when_tuning_differs() {
        let mut other = tuning();
        other.hop_size = 20.0;
        let a = route_fingerprint("45.0,-73.0", "43.0,-79.0", &tuning());
        let b = route_fingerprint("45.0,-73.0", "43.0,-79.0", &other);
        assert_ne!(a, b);
    }

    #[test]
    fn route_fingerprint_differs_when_avoid_flags_differ() {
        let mut other = tuning();
        other.avoid_tolls = true;
        let a = route_fingerprint("45.0,-73.0", "43.0,-79.0", &tuning());
        let b = route_fingerprint("45.0,-73.0", "43.0,-79.0", &other);
        assert_ne!(a, b);
    }

    #[test]
    fn save_and_load_round_trips_the_itinerary() {
        let path = temp_path();
        let records = vec![record(1.0, 2.0, 3.0, "A", "OK", "\u{a9} Google")];
        let file = ItineraryFile {
            fingerprint: "abc".to_string(),
            records: records.clone(),
        };
        save_to(&path, &file).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded, Some(file));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_from_returns_none_when_file_does_not_exist() {
        let path = temp_path();
        assert_eq!(load_from(&path).unwrap(), None);
    }

    #[test]
    fn resolve_resume_starts_fresh_when_nothing_persisted() {
        let decision = resolve_resume(None, "fp1", false);
        assert!(matches!(decision, ResumeDecision::Fresh));
    }

    #[test]
    fn resolve_resume_resumes_on_matching_fingerprint() {
        let existing = ItineraryFile {
            fingerprint: "fp1".to_string(),
            records: vec![record(0.0, 0.0, 0.0, "A", "OK", "\u{a9} Google")],
        };
        let decision = resolve_resume(Some(existing.clone()), "fp1", false);
        match decision {
            ResumeDecision::Resume(records) => assert_eq!(records, existing.records),
            other => panic!("expected Resume, got {other:?}"),
        }
    }

    #[test]
    fn resolve_resume_rejects_mismatched_fingerprint_without_fresh() {
        let existing = ItineraryFile {
            fingerprint: "fp1".to_string(),
            records: vec![],
        };
        let decision = resolve_resume(Some(existing), "fp2", false);
        assert!(matches!(
            decision,
            ResumeDecision::FingerprintMismatch { .. }
        ));
    }

    #[test]
    fn resolve_resume_fresh_flag_bypasses_mismatch() {
        let existing = ItineraryFile {
            fingerprint: "fp1".to_string(),
            records: vec![],
        };
        let decision = resolve_resume(Some(existing), "fp2", true);
        assert!(matches!(decision, ResumeDecision::Fresh));
    }
}
