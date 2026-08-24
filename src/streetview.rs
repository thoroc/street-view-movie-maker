use serde::Deserialize;
use std::future::Future;

const METADATA_ENDPOINT: &str = "https://maps.googleapis.com/maps/api/streetview/metadata";
const IMAGE_ENDPOINT: &str = "https://maps.googleapis.com/maps/api/streetview";
const MAX_ATTEMPTS: u32 = 4;

#[derive(Debug, Clone)]
pub struct StreetviewParams {
    pub picsize: String,
    pub fov: u32,
    pub pitch: i32,
    pub radius: u32,
}

fn build_url(
    base: &str,
    api_key: &str,
    lat: f64,
    lon: f64,
    heading: f64,
    params: &StreetviewParams,
) -> String {
    let location = format!("{lat},{lon}");
    let query = [
        ("size", params.picsize.clone()),
        ("location", location),
        ("heading", heading.to_string()),
        ("fov", params.fov.to_string()),
        ("pitch", params.pitch.to_string()),
        ("radius", params.radius.to_string()),
        ("source", "outdoor".to_string()),
        ("key", api_key.to_string()),
    ];
    let query_string = query
        .iter()
        .map(|(k, v)| format!("{k}={}", crate::net::urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{base}?{query_string}")
}

pub fn build_metadata_url(
    api_key: &str,
    lat: f64,
    lon: f64,
    heading: f64,
    params: &StreetviewParams,
) -> String {
    build_url(METADATA_ENDPOINT, api_key, lat, lon, heading, params)
}

pub fn build_image_url(
    api_key: &str,
    lat: f64,
    lon: f64,
    heading: f64,
    params: &StreetviewParams,
) -> String {
    build_url(IMAGE_ENDPOINT, api_key, lat, lon, heading, params)
}

/// Redacts the `key=...` query parameter from a URL so it's safe to log or
/// include in an error message (see Secrets handling in the plan).
pub fn redact_key(url: &str) -> String {
    match url.find("key=") {
        Some(start) => {
            let end = url[start..]
                .find('&')
                .map(|i| start + i)
                .unwrap_or(url.len());
            format!("{}key=REDACTED{}", &url[..start], &url[end..])
        }
        None => url.to_string(),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StreetviewError {
    #[error("Street View API request denied (check your API key and enabled APIs): {0}")]
    Auth(String),
    #[error("Street View API quota exceeded")]
    QuotaExceeded,
    #[error("failed to parse Street View API response: {0}")]
    Parse(String),
    #[error("network error after {attempts} attempt(s): {message}")]
    Network { attempts: u32, message: String },
    #[error("failed to write downloaded image to {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Deserialize)]
struct RawMetadata {
    status: String,
    #[serde(default)]
    copyright: Option<String>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    pano_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreetviewMetadata {
    pub status: String,
    pub copyright: Option<String>,
    pub date: Option<String>,
    pub pano_id: Option<String>,
}

/// Parses a Street View metadata response. Most non-"OK" statuses (no
/// panorama nearby, invalid request, ...) are returned as an `Ok` result with
/// that status set, not an error — callers filter on `status == "OK"` plus
/// `is_from_google`, mirroring the Python original. Only account-level
/// failures (auth, quota) are treated as errors here.
pub fn parse_metadata_response(body: &str) -> Result<StreetviewMetadata, StreetviewError> {
    let raw: RawMetadata =
        serde_json::from_str(body).map_err(|e| StreetviewError::Parse(e.to_string()))?;
    match raw.status.as_str() {
        "REQUEST_DENIED" => Err(StreetviewError::Auth(raw.status)),
        "OVER_QUERY_LIMIT" => Err(StreetviewError::QuotaExceeded),
        _ => Ok(StreetviewMetadata {
            status: raw.status,
            copyright: raw.copyright,
            date: raw.date,
            pano_id: raw.pano_id,
        }),
    }
}

/// Mirrors the Python original's `'Google' in probe['copyright']` check.
pub fn is_from_google(copyright: &Option<String>) -> bool {
    copyright.as_deref().is_some_and(|c| c.contains("Google"))
}

#[derive(Debug)]
enum FetchError {
    Transient(String),
    Fatal(String),
}

async fn fetch_with_retry(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, StreetviewError> {
    let cache_dir = crate::net::http_cache_dir();
    crate::net::cached_fetch(url, cache_dir.as_deref(), || {
        fetch_with_retry_live(client, url)
    })
    .await
}

async fn fetch_with_retry_live(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<u8>, StreetviewError> {
    let result = crate::net::with_retry(
        MAX_ATTEMPTS,
        |e: &FetchError| matches!(e, FetchError::Transient(_)),
        || async {
            let response = client.get(url).send().await.map_err(|e| {
                if e.is_timeout() || e.is_connect() {
                    FetchError::Transient(e.to_string())
                } else {
                    FetchError::Fatal(e.to_string())
                }
            })?;
            let status = response.status();
            if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(FetchError::Transient(format!("HTTP {status}")));
            }
            response
                .bytes()
                .await
                .map(|b| b.to_vec())
                .map_err(|e| FetchError::Fatal(e.to_string()))
        },
    )
    .await;

    result.map_err(|(attempts, err)| {
        let detail = match err {
            FetchError::Transient(m) | FetchError::Fatal(m) => m,
        };
        // Redact the key even though `detail` shouldn't contain it — this is
        // the request URL, and it's cheap insurance against a future error
        // path that echoes it back verbatim.
        let message = format!("{detail} (request: {})", redact_key(url));
        StreetviewError::Network { attempts, message }
    })
}

/// Probes Street View metadata for one point/heading. Not unit tested here —
/// exercised by the end-to-end test (see the plan's Testing strategy); the
/// pure URL-building/parsing/redaction logic above carries this module's
/// unit test coverage.
pub async fn probe_metadata(
    client: &reqwest::Client,
    api_key: &str,
    lat: f64,
    lon: f64,
    heading: f64,
    params: &StreetviewParams,
) -> Result<StreetviewMetadata, StreetviewError> {
    let url = build_metadata_url(api_key, lat, lon, heading, params);
    let bytes = fetch_with_retry(client, &url).await?;
    let body = String::from_utf8_lossy(&bytes);
    parse_metadata_response(&body)
}

/// Downloads one Street View image to `dest_path`. See `probe_metadata` for
/// why this isn't independently unit tested.
pub async fn download_image(
    client: &reqwest::Client,
    api_key: &str,
    lat: f64,
    lon: f64,
    heading: f64,
    params: &StreetviewParams,
    dest_path: &std::path::Path,
) -> Result<(), StreetviewError> {
    let url = build_image_url(api_key, lat, lon, heading, params);
    let bytes = fetch_with_retry(client, &url).await?;
    tokio::fs::write(dest_path, bytes)
        .await
        .map_err(|source| StreetviewError::Io {
            path: dest_path.display().to_string(),
            source,
        })
}

/// Runs `f` over `items` with at most `limit` concurrently in flight,
/// returning results in the original order. Backs the `--concurrency` flag.
pub async fn run_bounded<T, R, Fut>(items: Vec<T>, limit: usize, f: impl Fn(T) -> Fut) -> Vec<R>
where
    T: Send + 'static,
    R: Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
{
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    let semaphore = Arc::new(Semaphore::new(limit.max(1)));
    let mut set = tokio::task::JoinSet::new();
    for (index, item) in items.into_iter().enumerate() {
        let semaphore = semaphore.clone();
        let fut = f(item);
        set.spawn(async move {
            let _permit = semaphore.acquire_owned().await.expect("semaphore closed");
            (index, fut.await)
        });
    }

    let mut results: Vec<Option<R>> = (0..set.len()).map(|_| None).collect();
    while let Some(joined) = set.join_next().await {
        let (index, value) = joined.expect("task panicked");
        results[index] = Some(value);
    }
    results
        .into_iter()
        .map(|r| r.expect("all indices filled"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> StreetviewParams {
        StreetviewParams {
            picsize: "640x320".to_string(),
            fov: 90,
            pitch: 0,
            radius: 5,
        }
    }

    #[test]
    fn builds_metadata_url_with_expected_query_params() {
        let url = build_metadata_url("KEY123", 45.5, -73.5, 90.0, &params());
        assert!(url.starts_with("https://maps.googleapis.com/maps/api/streetview/metadata?"));
        assert!(url.contains("location=45.5%2C-73.5"));
        assert!(url.contains("heading=90"));
        assert!(url.contains("fov=90"));
        assert!(url.contains("pitch=0"));
        assert!(url.contains("radius=5"));
        assert!(url.contains("key=KEY123"));
    }

    #[test]
    fn builds_image_url_with_expected_query_params() {
        let url = build_image_url("KEY123", 45.5, -73.5, 90.0, &params());
        assert!(url.starts_with("https://maps.googleapis.com/maps/api/streetview?"));
        assert!(url.contains("size=640x320"));
        assert!(url.contains("location=45.5%2C-73.5"));
        assert!(url.contains("key=KEY123"));
    }

    #[test]
    fn redact_key_hides_the_api_key_value() {
        let url = "https://example.com/x?location=1,2&key=SUPER_SECRET&fov=90";
        let redacted = redact_key(url);
        assert!(!redacted.contains("SUPER_SECRET"));
        assert!(redacted.contains("key=REDACTED"));
        assert!(redacted.contains("location=1,2"));
        assert!(redacted.contains("fov=90"));
    }

    #[test]
    fn redact_key_is_a_no_op_when_there_is_no_key_param() {
        let url = "https://example.com/x?location=1,2";
        assert_eq!(redact_key(url), url);
    }

    const OK_METADATA: &str = r#"{
        "status": "OK",
        "copyright": "© Google",
        "date": "2019-07",
        "pano_id": "abc123",
        "location": {"lat": 45.5, "lng": -73.5}
    }"#;

    #[test]
    fn parses_ok_metadata_response() {
        let meta = parse_metadata_response(OK_METADATA).unwrap();
        assert_eq!(meta.status, "OK");
        assert_eq!(meta.pano_id.as_deref(), Some("abc123"));
        assert_eq!(meta.copyright.as_deref(), Some("\u{a9} Google"));
    }

    #[test]
    fn parses_zero_results_metadata_as_ok_result_not_error() {
        let body = r#"{"status": "ZERO_RESULTS"}"#;
        let meta = parse_metadata_response(body).unwrap();
        assert_eq!(meta.status, "ZERO_RESULTS");
        assert_eq!(meta.pano_id, None);
    }

    #[test]
    fn request_denied_metadata_status_maps_to_auth_error() {
        let body = r#"{"status": "REQUEST_DENIED"}"#;
        let err = parse_metadata_response(body).unwrap_err();
        assert!(matches!(err, StreetviewError::Auth(_)));
    }

    #[test]
    fn over_query_limit_metadata_status_maps_to_quota_exceeded() {
        let body = r#"{"status": "OVER_QUERY_LIMIT"}"#;
        let err = parse_metadata_response(body).unwrap_err();
        assert!(matches!(err, StreetviewError::QuotaExceeded));
    }

    #[test]
    fn malformed_metadata_json_maps_to_parse_error() {
        let err = parse_metadata_response("not json").unwrap_err();
        assert!(matches!(err, StreetviewError::Parse(_)));
    }

    #[test]
    fn is_from_google_true_for_google_copyright() {
        assert!(is_from_google(&Some("\u{a9} Google".to_string())));
    }

    #[test]
    fn is_from_google_false_for_third_party_copyright() {
        assert!(!is_from_google(&Some(
            "\u{a9} Jane Doe (contributor)".to_string()
        )));
    }

    #[test]
    fn is_from_google_false_when_missing() {
        assert!(!is_from_google(&None));
    }

    #[tokio::test]
    async fn run_bounded_never_exceeds_the_concurrency_limit() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let items: Vec<usize> = (0..10).collect();

        let results = run_bounded(items, 3, {
            let in_flight = in_flight.clone();
            let peak = peak.clone();
            move |item| {
                let in_flight = in_flight.clone();
                let peak = peak.clone();
                async move {
                    let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(current, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    item * 2
                }
            }
        })
        .await;

        assert_eq!(results.len(), 10);
        assert_eq!(results[5], 10);
        assert!(peak.load(Ordering::SeqCst) <= 3);
    }
}
