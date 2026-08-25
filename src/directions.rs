use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum RouteEndpoint {
    Coordinates(f64, f64),
    PlaceName(String),
}

impl RouteEndpoint {
    pub fn as_directions_param(&self) -> String {
        match self {
            RouteEndpoint::Coordinates(lat, lon) => format!("{lat},{lon}"),
            RouteEndpoint::PlaceName(name) => name.clone(),
        }
    }
}

/// Parses a `--from`/`--to` value as `"lat,lon"`; anything else is treated as a
/// free-text place name to be resolved by the Directions API itself.
pub fn parse_route_endpoint(input: &str) -> RouteEndpoint {
    if let Some((lat_str, lon_str)) = input.split_once(',')
        && let (Ok(lat), Ok(lon)) = (lat_str.trim().parse::<f64>(), lon_str.trim().parse::<f64>())
    {
        return RouteEndpoint::Coordinates(lat, lon);
    }
    RouteEndpoint::PlaceName(input.to_string())
}

/// Mirrors Google Maps' own "Avoid tolls/highways/ferries" route toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvoidFeature {
    Tolls,
    Highways,
    Ferries,
}

impl AvoidFeature {
    fn as_query_token(&self) -> &'static str {
        match self {
            AvoidFeature::Tolls => "tolls",
            AvoidFeature::Highways => "highways",
            AvoidFeature::Ferries => "ferries",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DirectionsError {
    #[error("Directions API request denied (check your API key and enabled APIs): {0}")]
    Auth(String),
    #[error("Directions API quota exceeded")]
    QuotaExceeded,
    #[error("Directions API returned no usable route: {0}")]
    NoRoute(String),
    #[error("failed to parse Directions API response: {0}")]
    Parse(String),
    #[error("failed to decode route polyline: {0}")]
    Polyline(String),
    #[error("network error after {attempts} attempt(s): {message}")]
    Network { attempts: u32, message: String },
}

#[derive(Debug, Deserialize)]
struct DirectionsResponse {
    status: String,
    #[serde(default)]
    error_message: Option<String>,
    routes: Vec<RawRoute>,
}

#[derive(Debug, Deserialize)]
struct RawRoute {
    overview_polyline: OverviewPolyline,
    legs: Vec<Leg>,
}

#[derive(Debug, Deserialize)]
struct OverviewPolyline {
    points: String,
}

#[derive(Debug, Deserialize)]
struct Leg {
    start_location: LatLng,
    end_location: LatLng,
    start_address: Option<String>,
    end_address: Option<String>,
    #[serde(default)]
    steps: Vec<RawStep>,
}

#[derive(Debug, Deserialize)]
struct RawStep {
    html_instructions: String,
    #[serde(default)]
    maneuver: Option<String>,
    start_location: LatLng,
}

#[derive(Debug, Deserialize)]
struct LatLng {
    lat: f64,
    lng: f64,
}

/// A small display set Google's `maneuver` strings collapse into. `Other`
/// covers both known non-directional maneuvers (`merge`, `ferry`) and any
/// maneuver string not yet recognized here — the sign still shows (see the
/// straight-on decision in the plan), just without a left/right glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnDirection {
    Left,
    Right,
    StraightOn,
    Other,
}

impl TurnDirection {
    fn from_maneuver(maneuver: &str) -> Self {
        match maneuver {
            "turn-left" | "turn-slight-left" | "turn-sharp-left" | "uturn-left" | "fork-left"
            | "keep-left" | "roundabout-left" | "ramp-left" => TurnDirection::Left,
            "turn-right" | "turn-slight-right" | "turn-sharp-right" | "uturn-right"
            | "fork-right" | "keep-right" | "roundabout-right" | "ramp-right" => {
                TurnDirection::Right
            }
            "straight" => TurnDirection::StraightOn,
            "merge" | "ferry" | "ferry-train" => TurnDirection::Other,
            other => {
                eprintln!(
                    "unrecognized Directions API maneuver {other:?} — showing a sign with no direction glyph"
                );
                TurnDirection::Other
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Maneuver {
    pub at: (f64, f64),
    pub direction: TurnDirection,
    pub road_name: Option<String>,
}

/// Strips HTML tags from a Directions API `html_instructions` string, e.g.
/// `"Turn <b>left</b> onto <b>Main St</b>"` -> `"Turn left onto Main St"`.
fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Extracts a road name from a step's `html_instructions`: the text after
/// "onto"/"on" when present, else the tag-stripped instruction as a
/// fallback. Returns `None` only if stripping leaves nothing at all.
fn extract_road_name(html_instructions: &str) -> Option<String> {
    let stripped = strip_html_tags(html_instructions);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return None;
    }
    for marker in ["onto ", " on "] {
        if let Some(idx) = trimmed.find(marker) {
            let after = trimmed[idx + marker.len()..].trim();
            if !after.is_empty() {
                return Some(after.to_string());
            }
        }
    }
    Some(trimmed.to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRoute {
    pub points: Vec<(f64, f64)>,
    pub start: (f64, f64),
    pub end: (f64, f64),
    pub start_address: Option<String>,
    pub end_address: Option<String>,
    pub maneuvers: Vec<Maneuver>,
}

/// Parses a raw Directions API JSON body into a resolved route, or a typed
/// error distinguishing auth/quota/no-route/parse failures (see Error handling
/// in the plan) so callers can decide what's retryable.
pub fn parse_directions_response(body: &str) -> Result<ResolvedRoute, DirectionsError> {
    let response: DirectionsResponse =
        serde_json::from_str(body).map_err(|e| DirectionsError::Parse(e.to_string()))?;

    match response.status.as_str() {
        "OK" => {}
        "REQUEST_DENIED" => {
            return Err(DirectionsError::Auth(
                response.error_message.unwrap_or(response.status),
            ));
        }
        "OVER_QUERY_LIMIT" => return Err(DirectionsError::QuotaExceeded),
        other => {
            return Err(DirectionsError::NoRoute(
                response.error_message.unwrap_or_else(|| other.to_string()),
            ));
        }
    }

    let route = response
        .routes
        .first()
        .ok_or_else(|| DirectionsError::NoRoute("no routes in response".to_string()))?;
    let leg = route
        .legs
        .first()
        .ok_or_else(|| DirectionsError::NoRoute("no legs in route".to_string()))?;

    let decoded = polyline::decode_polyline(&route.overview_polyline.points, 5)
        .map_err(|e| DirectionsError::Polyline(e.to_string()))?;
    let points: Vec<(f64, f64)> = decoded.coords().map(|c| (c.y, c.x)).collect();

    // Single leg only: this project has no waypoint/multi-stop support, so a
    // point-A-to-B request always returns exactly one leg (see the plan's
    // Out of scope section) — no multi-leg merge to reason about here.
    let maneuvers: Vec<Maneuver> = leg
        .steps
        .iter()
        .filter_map(|step| {
            let maneuver = step.maneuver.as_deref()?;
            Some(Maneuver {
                at: (step.start_location.lat, step.start_location.lng),
                direction: TurnDirection::from_maneuver(maneuver),
                road_name: extract_road_name(&step.html_instructions),
            })
        })
        .collect();

    Ok(ResolvedRoute {
        points,
        start: (leg.start_location.lat, leg.start_location.lng),
        end: (leg.end_location.lat, leg.end_location.lng),
        start_address: leg.start_address.clone(),
        end_address: leg.end_address.clone(),
        maneuvers,
    })
}

const DIRECTIONS_ENDPOINT: &str = "https://maps.googleapis.com/maps/api/directions/json";
const MAX_ATTEMPTS: u32 = 4;

/// Builds the Directions API query params, omitting `avoid` entirely when
/// no feature is selected (matching "avoid nothing" as the default). Kept
/// pure and separate from `fetch_directions` so it's unit testable without a
/// network call (mirrors `streetview::build_url`).
fn build_directions_params(
    origin: &RouteEndpoint,
    destination: &RouteEndpoint,
    api_key: &str,
    avoid: &[AvoidFeature],
) -> Vec<(&'static str, String)> {
    let mut params = vec![
        ("origin", origin.as_directions_param()),
        ("destination", destination.as_directions_param()),
        ("mode", "driving".to_string()),
        ("key", api_key.to_string()),
    ];
    if !avoid.is_empty() {
        let joined = avoid
            .iter()
            .map(AvoidFeature::as_query_token)
            .collect::<Vec<_>>()
            .join("|");
        params.push(("avoid", joined));
    }
    params
}

#[derive(Debug)]
enum FetchError {
    Transient(String),
    Fatal(String),
}

fn build_directions_url(params: &[(&'static str, String)]) -> String {
    let query_string = params
        .iter()
        .map(|(k, v)| format!("{k}={}", crate::net::urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{DIRECTIONS_ENDPOINT}?{query_string}")
}

/// Fetches and resolves a route from the Directions API, retrying transient
/// failures (timeouts, connect errors, 5xx, 429) with exponential backoff via
/// `net::with_retry`. Not unit tested here — exercised by the end-to-end test
/// against the real API (see the plan's Testing strategy); the pure
/// parsing/error-mapping logic above is what carries this module's unit test
/// coverage.
pub async fn fetch_directions(
    client: &reqwest::Client,
    api_key: &str,
    origin: &RouteEndpoint,
    destination: &RouteEndpoint,
    avoid: &[AvoidFeature],
) -> Result<ResolvedRoute, DirectionsError> {
    let params = build_directions_params(origin, destination, api_key, avoid);
    let url = build_directions_url(&params);

    let cache_dir = crate::net::http_cache_dir();
    let body_bytes = crate::net::cached_fetch(&url, cache_dir.as_deref(), || {
        fetch_directions_body(client, &url)
    })
    .await?;
    let body = String::from_utf8(body_bytes).map_err(|e| DirectionsError::Parse(e.to_string()))?;
    parse_directions_response(&body)
}

async fn fetch_directions_body(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<u8>, DirectionsError> {
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

    match result {
        Ok(bytes) => Ok(bytes),
        Err((attempts, FetchError::Transient(message) | FetchError::Fatal(message))) => {
            Err(DirectionsError::Network { attempts, message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, epsilon: f64) {
        assert!(
            (actual - expected).abs() < epsilon,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn parses_lat_lon_pair_as_coordinates() {
        match parse_route_endpoint("45.517146,-73.579837") {
            RouteEndpoint::Coordinates(lat, lon) => {
                assert_close(lat, 45.517146, 1e-9);
                assert_close(lon, -73.579837, 1e-9);
            }
            other => panic!("expected Coordinates, got {other:?}"),
        }
    }

    #[test]
    fn trims_whitespace_around_coordinates() {
        match parse_route_endpoint(" 45.5 , -73.5 ") {
            RouteEndpoint::Coordinates(lat, lon) => {
                assert_close(lat, 45.5, 1e-9);
                assert_close(lon, -73.5, 1e-9);
            }
            other => panic!("expected Coordinates, got {other:?}"),
        }
    }

    #[test]
    fn falls_back_to_place_name_when_not_coordinates() {
        match parse_route_endpoint("Barfly, Montreal") {
            RouteEndpoint::PlaceName(name) => assert_eq!(name, "Barfly, Montreal"),
            other => panic!("expected PlaceName, got {other:?}"),
        }
    }

    #[test]
    fn falls_back_to_place_name_for_single_number_with_no_comma() {
        match parse_route_endpoint("42") {
            RouteEndpoint::PlaceName(name) => assert_eq!(name, "42"),
            other => panic!("expected PlaceName, got {other:?}"),
        }
    }

    #[test]
    fn as_directions_param_formats_coordinates() {
        let endpoint = RouteEndpoint::Coordinates(45.5, -73.5);
        assert_eq!(endpoint.as_directions_param(), "45.5,-73.5");
    }

    #[test]
    fn as_directions_param_passes_place_name_through() {
        let endpoint = RouteEndpoint::PlaceName("Barfly, Montreal".to_string());
        assert_eq!(endpoint.as_directions_param(), "Barfly, Montreal");
    }

    const OK_RESPONSE: &str = r#"{
        "status": "OK",
        "routes": [{
            "overview_polyline": {"points": "_p~iF~ps|U_ulLnnqC_mqNvxq`@"},
            "legs": [{
                "start_location": {"lat": 38.5, "lng": -120.2},
                "end_location": {"lat": 43.25200, "lng": -126.45300},
                "start_address": "Start Address",
                "end_address": "End Address"
            }]
        }]
    }"#;

    #[test]
    fn parses_ok_response_into_resolved_route() {
        let route = parse_directions_response(OK_RESPONSE).unwrap();
        assert_close(route.start.0, 38.5, 1e-9);
        assert_close(route.start.1, -120.2, 1e-9);
        assert_close(route.end.0, 43.252, 1e-9);
        assert_close(route.end.1, -126.453, 1e-9);
        assert_eq!(route.start_address.as_deref(), Some("Start Address"));
        assert_eq!(route.end_address.as_deref(), Some("End Address"));
    }

    #[test]
    fn decodes_polyline_into_expected_points() {
        let route = parse_directions_response(OK_RESPONSE).unwrap();
        assert_eq!(route.points.len(), 3);
        assert_close(route.points[0].0, 38.5, 1e-4);
        assert_close(route.points[0].1, -120.2, 1e-4);
        assert_close(route.points[1].0, 40.7, 1e-4);
        assert_close(route.points[1].1, -120.95, 1e-4);
        assert_close(route.points[2].0, 43.252, 1e-4);
        assert_close(route.points[2].1, -126.453, 1e-4);
    }

    #[test]
    fn request_denied_status_maps_to_auth_error() {
        let body = r#"{"status": "REQUEST_DENIED", "error_message": "bad key", "routes": []}"#;
        let err = parse_directions_response(body).unwrap_err();
        assert!(matches!(err, DirectionsError::Auth(_)));
    }

    #[test]
    fn over_query_limit_status_maps_to_quota_exceeded() {
        let body = r#"{"status": "OVER_QUERY_LIMIT", "routes": []}"#;
        let err = parse_directions_response(body).unwrap_err();
        assert!(matches!(err, DirectionsError::QuotaExceeded));
    }

    #[test]
    fn zero_results_status_maps_to_no_route() {
        let body = r#"{"status": "ZERO_RESULTS", "routes": []}"#;
        let err = parse_directions_response(body).unwrap_err();
        assert!(matches!(err, DirectionsError::NoRoute(_)));
    }

    #[test]
    fn malformed_json_maps_to_parse_error() {
        let err = parse_directions_response("not json").unwrap_err();
        assert!(matches!(err, DirectionsError::Parse(_)));
    }

    #[test]
    fn maneuver_mapping_table() {
        let left = [
            "turn-left",
            "turn-slight-left",
            "turn-sharp-left",
            "uturn-left",
            "fork-left",
            "keep-left",
            "roundabout-left",
            "ramp-left",
        ];
        for maneuver in left {
            assert_eq!(
                TurnDirection::from_maneuver(maneuver),
                TurnDirection::Left,
                "expected {maneuver} to map to Left"
            );
        }

        let right = [
            "turn-right",
            "turn-slight-right",
            "turn-sharp-right",
            "uturn-right",
            "fork-right",
            "keep-right",
            "roundabout-right",
            "ramp-right",
        ];
        for maneuver in right {
            assert_eq!(
                TurnDirection::from_maneuver(maneuver),
                TurnDirection::Right,
                "expected {maneuver} to map to Right"
            );
        }

        assert_eq!(
            TurnDirection::from_maneuver("straight"),
            TurnDirection::StraightOn
        );
    }

    #[test]
    fn maneuver_mapping_falls_back_to_other_for_non_directional_and_unknown_values() {
        for maneuver in ["merge", "ferry", "ferry-train", "some-future-maneuver"] {
            assert_eq!(
                TurnDirection::from_maneuver(maneuver),
                TurnDirection::Other,
                "expected {maneuver} to map to Other"
            );
        }
    }

    #[test]
    fn extracts_road_name_after_onto() {
        assert_eq!(
            extract_road_name("Turn <b>left</b> onto <b>Main St</b>"),
            Some("Main St".to_string())
        );
    }

    #[test]
    fn extracts_road_name_after_on() {
        assert_eq!(
            extract_road_name("Continue <b>straight</b> on <b>Elm St</b>"),
            Some("Elm St".to_string())
        );
    }

    #[test]
    fn extracts_road_name_falls_back_to_stripped_instruction() {
        assert_eq!(
            extract_road_name("Head <b>north</b>"),
            Some("Head north".to_string())
        );
    }

    #[test]
    fn extracts_road_name_returns_none_for_empty_instruction() {
        assert_eq!(extract_road_name("<b></b>"), None);
    }

    const STEPS_RESPONSE: &str = r#"{
        "status": "OK",
        "routes": [{
            "overview_polyline": {"points": "_p~iF~ps|U_ulLnnqC_mqNvxq`@"},
            "legs": [{
                "start_location": {"lat": 38.5, "lng": -120.2},
                "end_location": {"lat": 43.25200, "lng": -126.45300},
                "start_address": "Start Address",
                "end_address": "End Address",
                "steps": [
                    {
                        "html_instructions": "Head <b>north</b>",
                        "start_location": {"lat": 38.5, "lng": -120.2}
                    },
                    {
                        "html_instructions": "Turn <b>left</b> onto <b>Main St</b>",
                        "maneuver": "turn-left",
                        "start_location": {"lat": 40.7, "lng": -120.95}
                    },
                    {
                        "html_instructions": "Continue <b>straight</b> onto <b>Main St</b>",
                        "maneuver": "straight",
                        "start_location": {"lat": 41.0, "lng": -121.0}
                    }
                ]
            }]
        }]
    }"#;

    #[test]
    fn parses_maneuvers_from_steps_skipping_steps_with_no_maneuver_field() {
        let route = parse_directions_response(STEPS_RESPONSE).unwrap();
        assert_eq!(route.maneuvers.len(), 2);
        assert_eq!(route.maneuvers[0].direction, TurnDirection::Left);
        assert_eq!(route.maneuvers[0].road_name.as_deref(), Some("Main St"));
        assert_eq!(route.maneuvers[1].direction, TurnDirection::StraightOn);
    }

    #[test]
    fn ok_response_with_no_steps_has_no_maneuvers() {
        let route = parse_directions_response(OK_RESPONSE).unwrap();
        assert!(route.maneuvers.is_empty());
    }

    fn find_avoid_param<'a>(params: &'a [(&'static str, String)]) -> Option<&'a str> {
        params
            .iter()
            .find(|(key, _)| *key == "avoid")
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn build_directions_params_omits_avoid_when_no_feature_selected() {
        let origin = RouteEndpoint::Coordinates(45.0, -73.0);
        let destination = RouteEndpoint::Coordinates(43.0, -79.0);
        let params = build_directions_params(&origin, &destination, "key", &[]);
        assert_eq!(find_avoid_param(&params), None);
    }

    #[test]
    fn build_directions_params_includes_single_avoid_token() {
        let origin = RouteEndpoint::Coordinates(45.0, -73.0);
        let destination = RouteEndpoint::Coordinates(43.0, -79.0);
        let params = build_directions_params(&origin, &destination, "key", &[AvoidFeature::Tolls]);
        assert_eq!(find_avoid_param(&params), Some("tolls"));
    }

    #[test]
    fn build_directions_params_pipe_joins_multiple_avoid_tokens() {
        let origin = RouteEndpoint::Coordinates(45.0, -73.0);
        let destination = RouteEndpoint::Coordinates(43.0, -79.0);
        let params = build_directions_params(
            &origin,
            &destination,
            "key",
            &[
                AvoidFeature::Tolls,
                AvoidFeature::Highways,
                AvoidFeature::Ferries,
            ],
        );
        assert_eq!(find_avoid_param(&params), Some("tolls|highways|ferries"));
    }
}
