use serde::Deserialize;

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
}

#[derive(Debug, Deserialize)]
struct LatLng {
    lat: f64,
    lng: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRoute {
    pub points: Vec<(f64, f64)>,
    pub start: (f64, f64),
    pub end: (f64, f64),
    pub start_address: Option<String>,
    pub end_address: Option<String>,
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

    Ok(ResolvedRoute {
        points,
        start: (leg.start_location.lat, leg.start_location.lng),
        end: (leg.end_location.lat, leg.end_location.lng),
        start_address: leg.start_address.clone(),
        end_address: leg.end_address.clone(),
    })
}

const DIRECTIONS_ENDPOINT: &str = "https://maps.googleapis.com/maps/api/directions/json";
const MAX_ATTEMPTS: u32 = 4;

fn backoff_delay(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_millis(200 * 2u64.pow(attempt.min(5)))
}

/// Fetches and resolves a route from the Directions API, retrying transient
/// failures (timeouts, connect errors, 5xx, 429) with exponential backoff.
/// Not unit tested here — exercised by the end-to-end test against the real
/// API (see the plan's Testing strategy); the pure parsing/error-mapping
/// logic above is what carries this module's unit test coverage.
pub async fn fetch_directions(
    client: &reqwest::Client,
    api_key: &str,
    origin: &RouteEndpoint,
    destination: &RouteEndpoint,
) -> Result<ResolvedRoute, DirectionsError> {
    let params = [
        ("origin", origin.as_directions_param()),
        ("destination", destination.as_directions_param()),
        ("mode", "driving".to_string()),
        ("key", api_key.to_string()),
    ];

    let mut attempt = 0;
    loop {
        attempt += 1;
        match client.get(DIRECTIONS_ENDPOINT).query(&params).send().await {
            Ok(response) => {
                let status = response.status();
                let transient =
                    status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS;
                if transient && attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(backoff_delay(attempt)).await;
                    continue;
                }
                if transient {
                    return Err(DirectionsError::Network {
                        attempts: attempt,
                        message: format!("HTTP {status}"),
                    });
                }
                let body = response
                    .text()
                    .await
                    .map_err(|e| DirectionsError::Network {
                        attempts: attempt,
                        message: e.to_string(),
                    })?;
                return parse_directions_response(&body);
            }
            Err(e) if (e.is_timeout() || e.is_connect()) && attempt < MAX_ATTEMPTS => {
                tokio::time::sleep(backoff_delay(attempt)).await;
            }
            Err(e) => {
                return Err(DirectionsError::Network {
                    attempts: attempt,
                    message: e.to_string(),
                });
            }
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
}
