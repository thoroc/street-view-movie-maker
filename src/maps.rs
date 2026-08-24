use geo_types::Coord;
use std::path::{Path, PathBuf};

const STATIC_MAP_ENDPOINT: &str = "https://maps.googleapis.com/maps/api/staticmap";
const MAX_ATTEMPTS: u32 = 4;

/// Google doesn't document an explicit path-length limit for the Static Maps
/// API; 8192 is the commonly-cited safe ceiling for URLs in general (many
/// browsers/servers), used here as the simplification target so the request
/// stays well within it regardless of route length.
const STATIC_MAPS_URL_LIMIT: usize = 8192;

pub const DEFAULT_PATH_COLOR: &str = "0x0000ffff";
pub const DEFAULT_PATH_WEIGHT: u32 = 3;

/// The one Static Maps request this run makes is always fetched at this
/// fixed resolution — 640x640, the free/standard-tier maximum at `scale=1`
/// — regardless of `--map-size`. `--map-size` instead sets the size of the
/// local-area window panned/cropped out of this base image, centered on the
/// current position, per frame (see `compositing::crop_window`). Fetching
/// bigger than the crop needs gives that pan/crop room to move while
/// keeping local detail sharp, without needing more than the one API call.
pub const BASE_MAP_SIZE: (u32, u32) = (640, 640);

/// Everything needed to draw the route path on the one Static Maps request
/// this run makes — grouped so the request-building functions below don't
/// balloon into a long positional argument list. All fields are `Copy` so
/// `build_static_map_url_fitted` can rebuild one with a simplified `points`
/// slice via struct-update syntax without re-threading every other field.
#[derive(Clone, Copy)]
pub struct StaticMapRequest<'a> {
    pub size: (u32, u32),
    pub zoom: u32,
    pub center: (f64, f64),
    pub points: &'a [(f64, f64)],
    pub color: &'a str,
    pub weight: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum MapsError {
    #[error(
        "Maps Static API request denied (needs the Maps Static API enabled for this project): {0}"
    )]
    Auth(String),
    #[error("failed to encode route polyline: {0}")]
    Polyline(String),
    #[error("invalid --map-size value {0:?} (expected e.g. \"200x200\")")]
    InvalidSize(String),
    #[error("network error after {attempts} attempt(s): {message}")]
    Network { attempts: u32, message: String },
    #[error("failed to write map image to {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Parses a `--map-size`-style value ("200x200") into (width, height).
pub fn parse_size(value: &str) -> Result<(u32, u32), MapsError> {
    let (w, h) = value
        .split_once('x')
        .ok_or_else(|| MapsError::InvalidSize(value.to_string()))?;
    let width: u32 = w
        .parse()
        .map_err(|_| MapsError::InvalidSize(value.to_string()))?;
    let height: u32 = h
        .parse()
        .map_err(|_| MapsError::InvalidSize(value.to_string()))?;
    Ok((width, height))
}

fn perpendicular_distance(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let (x, y) = point;
    let (x1, y1) = start;
    let (x2, y2) = end;
    let dx = x2 - x1;
    let dy = y2 - y1;
    if dx == 0.0 && dy == 0.0 {
        return ((x - x1).powi(2) + (y - y1).powi(2)).sqrt();
    }
    ((dy * x - dx * y + x2 * y1 - y2 * x1).abs()) / (dx * dx + dy * dy).sqrt()
}

fn douglas_peucker(
    points: &[(f64, f64)],
    start: usize,
    end: usize,
    epsilon: f64,
    keep: &mut [bool],
) {
    if end <= start + 1 {
        return;
    }
    let (mut max_dist, mut index) = (0.0, start);
    for i in (start + 1)..end {
        let dist = perpendicular_distance(points[i], points[start], points[end]);
        if dist > max_dist {
            max_dist = dist;
            index = i;
        }
    }
    if max_dist > epsilon {
        keep[index] = true;
        douglas_peucker(points, start, index, epsilon, keep);
        douglas_peucker(points, index, end, epsilon, keep);
    }
}

/// Douglas-Peucker point reduction, so a long route's path still renders
/// correctly in a Static Maps URL instead of being truncated or overflowing
/// the URL length budget.
pub fn simplify_polyline(points: &[(f64, f64)], epsilon: f64) -> Vec<(f64, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    douglas_peucker(points, 0, points.len() - 1, epsilon, &mut keep);
    points
        .iter()
        .zip(keep)
        .filter_map(|(&p, k)| k.then_some(p))
        .collect()
}

fn encode_path(points: &[(f64, f64)]) -> Result<String, MapsError> {
    let coords = points.iter().map(|&(lat, lon)| Coord { x: lon, y: lat });
    polyline::encode_coordinates(coords, 5).map_err(|e| MapsError::Polyline(e.to_string()))
}

fn build_static_map_url(request: &StaticMapRequest, api_key: &str) -> Result<String, MapsError> {
    let encoded = encode_path(request.points)?;
    let (width, height) = request.size;
    let (lat, lon) = request.center;
    let (color, weight) = (request.color, request.weight);
    let query = [
        ("center", format!("{lat},{lon}")),
        ("zoom", request.zoom.to_string()),
        ("size", format!("{width}x{height}")),
        ("scale", "1".to_string()),
        (
            "path",
            format!("color:{color}|weight:{weight}|enc:{encoded}"),
        ),
        ("key", api_key.to_string()),
    ];
    let query_string = query
        .iter()
        .map(|(k, v)| format!("{k}={}", crate::net::urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    Ok(format!("{STATIC_MAP_ENDPOINT}?{query_string}"))
}

/// Builds a Static Maps URL for `request.points`, simplifying the path (see
/// `simplify_polyline`) as much as needed to stay under
/// `STATIC_MAPS_URL_LIMIT`, regardless of route length.
pub fn build_static_map_url_fitted(
    request: &StaticMapRequest,
    api_key: &str,
) -> Result<String, MapsError> {
    let mut epsilon = 0.000_01_f64;
    for _ in 0..40 {
        let simplified = simplify_polyline(request.points, epsilon);
        let url = build_static_map_url(
            &StaticMapRequest {
                points: &simplified,
                ..*request
            },
            api_key,
        )?;
        if url.len() <= STATIC_MAPS_URL_LIMIT || simplified.len() <= 2 {
            return Ok(url);
        }
        epsilon *= 2.0;
    }
    let endpoints = [request.points[0], request.points[request.points.len() - 1]];
    build_static_map_url(
        &StaticMapRequest {
            points: &endpoints,
            ..*request
        },
        api_key,
    )
}

#[derive(Debug)]
enum FetchError {
    Transient(String),
    Auth(String),
    Fatal(String),
}

async fn fetch_map_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, MapsError> {
    let cache_dir = crate::net::http_cache_dir();
    crate::net::cached_fetch(url, cache_dir.as_deref(), || {
        fetch_map_bytes_live(client, url)
    })
    .await
}

async fn fetch_map_bytes_live(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, MapsError> {
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
            if status == reqwest::StatusCode::FORBIDDEN {
                let body = response.text().await.unwrap_or_default();
                return Err(FetchError::Auth(body));
            }
            if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(FetchError::Transient(format!("HTTP {status}")));
            }
            if !status.is_success() {
                return Err(FetchError::Fatal(format!("HTTP {status}")));
            }
            response
                .bytes()
                .await
                .map(|b| b.to_vec())
                .map_err(|e| FetchError::Fatal(e.to_string()))
        },
    )
    .await;

    match result {
        Ok(bytes) => Ok(bytes),
        Err((_, FetchError::Auth(message))) => Err(MapsError::Auth(message)),
        Err((attempts, FetchError::Transient(message) | FetchError::Fatal(message))) => {
            Err(MapsError::Network { attempts, message })
        }
    }
}

/// Fetches and caches one Static Maps image for the whole run. If
/// `cache_path` already exists (e.g. a resumed run with an unchanged
/// `--map-size`), no network request is made at all — see the plan's
/// `map_<map-size>.png` cache-invalidation note.
pub async fn fetch_or_load_map(
    client: &reqwest::Client,
    api_key: &str,
    cache_path: &Path,
    request: &StaticMapRequest<'_>,
) -> Result<PathBuf, MapsError> {
    if cache_path.exists() {
        return Ok(cache_path.to_path_buf());
    }
    let url = build_static_map_url_fitted(request, api_key)?;
    let bytes = fetch_map_bytes(client, &url).await?;
    tokio::fs::write(cache_path, bytes)
        .await
        .map_err(|source| MapsError::Io {
            path: cache_path.display().to_string(),
            source,
        })?;
    Ok(cache_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_splits_width_and_height() {
        assert_eq!(parse_size("200x300").unwrap(), (200, 300));
    }

    #[test]
    fn parse_size_rejects_malformed_input() {
        assert!(parse_size("200").is_err());
        assert!(parse_size("axb").is_err());
    }

    #[test]
    fn simplify_polyline_keeps_endpoints() {
        let points = vec![(0.0, 0.0), (0.0001, 0.0001), (1.0, 1.0)];
        let simplified = simplify_polyline(&points, 10.0);
        assert_eq!(simplified.first(), points.first());
        assert_eq!(simplified.last(), points.last());
    }

    #[test]
    fn simplify_polyline_drops_near_collinear_points_at_large_epsilon() {
        let points = vec![(0.0, 0.0), (0.0, 0.5), (0.0, 1.0)];
        let simplified = simplify_polyline(&points, 1.0);
        assert_eq!(simplified.len(), 2);
    }

    #[test]
    fn build_static_map_url_fitted_stays_under_the_url_length_budget_for_a_long_route() {
        // A synthetic zig-zag with thousands of points, well beyond what a
        // naive un-simplified `enc:` path would fit in one URL.
        let points: Vec<(f64, f64)> = (0..5000)
            .map(|i| {
                let t = f64::from(i) * 0.0001;
                (t, if i % 2 == 0 { t } else { t + 0.00005 })
            })
            .collect();
        let request = StaticMapRequest {
            size: (200, 200),
            zoom: 10,
            center: (0.25, 0.25),
            points: &points,
            color: DEFAULT_PATH_COLOR,
            weight: DEFAULT_PATH_WEIGHT,
        };
        let url = build_static_map_url_fitted(&request, "KEY123").unwrap();
        assert!(
            url.len() <= STATIC_MAPS_URL_LIMIT,
            "url was {} bytes",
            url.len()
        );
    }

    #[test]
    fn build_static_map_url_fitted_includes_expected_query_params() {
        let points = vec![(45.5, -73.5), (45.6, -73.4)];
        let request = StaticMapRequest {
            size: (200, 200),
            zoom: 12,
            center: (45.55, -73.45),
            points: &points,
            color: DEFAULT_PATH_COLOR,
            weight: DEFAULT_PATH_WEIGHT,
        };
        let url = build_static_map_url_fitted(&request, "KEY123").unwrap();
        assert!(url.starts_with("https://maps.googleapis.com/maps/api/staticmap?"));
        assert!(url.contains("zoom=12"));
        assert!(url.contains("size=200x200"));
        assert!(url.contains("key=KEY123"));
        assert!(url.contains("path="));
    }
}
