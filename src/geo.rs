pub fn haversine_meters(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lon1) = (a.0.to_radians(), a.1.to_radians());
    let (lat2, lon2) = (b.0.to_radians(), b.1.to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * h.sqrt().asin() * 6_367_000.0
}

pub fn initial_compass_bearing(a: (f64, f64), b: (f64, f64)) -> f64 {
    let lat1 = a.0.to_radians();
    let lat2 = b.0.to_radians();
    let diff_long = (b.1 - a.1).to_radians();
    let x = diff_long.sin() * lat2.cos();
    let y = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * diff_long.cos();
    let initial_bearing = x.atan2(y).to_degrees();
    (initial_bearing + 360.0) % 360.0
}

fn linspace(start: f64, stop: f64, n: usize) -> Vec<f64> {
    if n <= 1 {
        return vec![start];
    }
    let step = (stop - start) / (n - 1) as f64;
    (0..n).map(|i| start + step * i as f64).collect()
}

pub fn interpolate_points(a: (f64, f64), b: (f64, f64), n_points: usize) -> Vec<(f64, f64)> {
    let lats = linspace(a.0, b.0, n_points);
    let lons = linspace(a.1, b.1, n_points);
    lats.into_iter().zip(lons).collect()
}

pub fn interpolate_points_by_hop(a: (f64, f64), b: (f64, f64), hop_size_m: f64) -> Vec<(f64, f64)> {
    let distance = haversine_meters(a, b);
    let n_points = (distance / hop_size_m).ceil() as usize;
    interpolate_points(a, b, n_points)
}

pub fn turn_headings(h1: f64, h2: f64, stepsize: f64) -> Vec<f64> {
    let mut h1 = h1;
    let mut h2 = h2;
    if h2 < h1 {
        h2 += 360.0;
    }
    let clockwise = h2 - h1 < 180.0;
    if !clockwise {
        h1 += 360.0;
    }
    let n_points = ((h1 - h2).abs() / stepsize).ceil() as usize;
    linspace(h1, h2, n_points)
        .into_iter()
        .map(|h| h.rem_euclid(360.0))
        .collect()
}

const TILE_SIZE: f64 = 256.0;
const MAX_ZOOM: u32 = 20;

fn lon_to_x(lon: f64) -> f64 {
    (lon + 180.0) / 360.0
}

fn lat_to_y(lat: f64) -> f64 {
    let sin_lat = lat.to_radians().sin();
    0.5 - ((1.0 + sin_lat) / (1.0 - sin_lat)).ln() / (4.0 * std::f64::consts::PI)
}

fn x_to_lon(x: f64) -> f64 {
    x * 360.0 - 180.0
}

fn y_to_lat(y: f64) -> f64 {
    let n = std::f64::consts::PI - 2.0 * std::f64::consts::PI * y;
    n.sinh().atan().to_degrees()
}

/// Picks the smallest bounding box (in Web Mercator space) around `points`
/// and the center/zoom needed to fit it within `width`x`height` at
/// `scale=1` — matching the Static Maps API's own tile math (256px tiles,
/// doubling per zoom level) so the same center/zoom pair used for the image
/// request can be reused for `lat_lon_to_pixel`'s marker placement.
///
/// Known limitation: no special case for a route crossing the antimeridian
/// (±180° longitude) — considered low-likelihood for this tool's typical
/// routes, so left unhandled rather than special-cased.
pub fn bbox_center_zoom(points: &[(f64, f64)], width: u32, height: u32) -> ((f64, f64), u32) {
    let mut lat_min = f64::INFINITY;
    let mut lat_max = f64::NEG_INFINITY;
    let mut lon_min = f64::INFINITY;
    let mut lon_max = f64::NEG_INFINITY;
    for &(lat, lon) in points {
        lat_min = lat_min.min(lat);
        lat_max = lat_max.max(lat);
        lon_min = lon_min.min(lon);
        lon_max = lon_max.max(lon);
    }

    let x_min = lon_to_x(lon_min);
    let x_max = lon_to_x(lon_max);
    let y_min = lat_to_y(lat_max);
    let y_max = lat_to_y(lat_min);

    let center = (
        y_to_lat((y_min + y_max) / 2.0),
        x_to_lon((x_min + x_max) / 2.0),
    );

    let mut zoom = 0;
    for z in 0..=MAX_ZOOM {
        let world_size = TILE_SIZE * 2f64.powi(z as i32);
        let px_width = (x_max - x_min) * world_size;
        let px_height = (y_max - y_min) * world_size;
        if px_width > f64::from(width) || px_height > f64::from(height) {
            break;
        }
        zoom = z;
    }

    (center, zoom)
}

/// Web-Mercator projection of `(lat, lon)` onto a `size`-pixel image centered
/// on `center` at `zoom`, matching how the Static Maps API renders a
/// `center`/`zoom`/`size` request. Assumes `scale=1` (no retina doubling) —
/// `size` is the requested Static Maps size in CSS pixels, not device
/// pixels; a "sharper inset" request using `scale=2` would need this helper
/// updated too, since device pixels would then be `2 * size`.
pub fn lat_lon_to_pixel(
    lat: f64,
    lon: f64,
    center: (f64, f64),
    zoom: u32,
    size: (u32, u32),
) -> (f64, f64) {
    let world_size = TILE_SIZE * 2f64.powi(zoom as i32);
    let point_x = lon_to_x(lon) * world_size;
    let point_y = lat_to_y(lat) * world_size;
    let center_x = lon_to_x(center.1) * world_size;
    let center_y = lat_to_y(center.0) * world_size;
    (
        point_x - center_x + f64::from(size.0) / 2.0,
        point_y - center_y + f64::from(size.1) / 2.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, epsilon: f64) {
        assert!(
            (actual - expected).abs() < epsilon,
            "expected {expected}, got {actual} (diff {})",
            (actual - expected).abs()
        );
    }

    const BARFLY: (f64, f64) = (45.517146, -73.579837);
    const DANFORTH: (f64, f64) = (43.676533, -79.357132);
    const JOSHUA_TREE_A: (f64, f64) = (33.669793, -115.802125);
    const JOSHUA_TREE_B: (f64, f64) = (33.671796, -115.801851);

    #[test]
    fn haversine_matches_python_reference_for_long_route() {
        assert_close(haversine_meters(BARFLY, DANFORTH), 500_661.534_753_7, 1e-3);
    }

    #[test]
    fn haversine_matches_python_reference_for_short_hop() {
        assert_close(
            haversine_meters(JOSHUA_TREE_A, JOSHUA_TREE_B),
            224.021_393_592_7,
            1e-6,
        );
    }

    #[test]
    fn haversine_of_a_point_with_itself_is_zero() {
        assert_close(haversine_meters(BARFLY, BARFLY), 0.0, 1e-9);
    }

    #[test]
    fn bearing_matches_python_reference_for_long_route() {
        assert_close(
            initial_compass_bearing(BARFLY, DANFORTH),
            247.943_466_561_522_77,
            1e-9,
        );
    }

    #[test]
    fn bearing_matches_python_reference_for_short_hop() {
        assert_close(
            initial_compass_bearing(JOSHUA_TREE_A, JOSHUA_TREE_B),
            6.494_837_398_830_043,
            1e-9,
        );
    }

    #[test]
    fn bearing_is_always_in_0_to_360_range() {
        let bearing = initial_compass_bearing(DANFORTH, BARFLY);
        assert!((0.0..360.0).contains(&bearing));
    }

    #[test]
    fn interpolate_points_matches_python_reference_for_fixed_count() {
        let points = interpolate_points(JOSHUA_TREE_A, JOSHUA_TREE_B, 5);
        let expected = [
            (33.669793, -115.802125),
            (33.670_293_75, -115.802_056_5),
            (33.670_794_5, -115.801_988),
            (33.671_295_25, -115.801_919_5),
            (33.671796, -115.801851),
        ];
        assert_eq!(points.len(), expected.len());
        for (i, (p, e)) in points.iter().zip(expected.iter()).enumerate() {
            assert_close(p.0, e.0, 1e-9);
            assert_close(p.1, e.1, 1e-9);
            let _ = i;
        }
    }

    #[test]
    fn interpolate_points_endpoints_are_exact() {
        let points = interpolate_points(JOSHUA_TREE_A, JOSHUA_TREE_B, 5);
        assert_eq!(*points.first().unwrap(), JOSHUA_TREE_A);
        assert_eq!(*points.last().unwrap(), JOSHUA_TREE_B);
    }

    #[test]
    fn interpolate_points_by_hop_matches_python_reference_count() {
        let points = interpolate_points_by_hop(JOSHUA_TREE_A, JOSHUA_TREE_B, 10.0);
        // Python reference: interpolate_points(c, d, hop_size=10) produced 23 points.
        assert_eq!(points.len(), 23);
        assert_close(points[0].0, 33.669793, 1e-9);
        assert_close(points.last().unwrap().0, 33.671796, 1e-9);
    }

    #[test]
    fn turn_headings_short_wrap_across_zero_reference() {
        // Python reference: get_turn_headings(10, 350, stepsize=15) == [10.0, 350.0]
        assert_eq!(turn_headings(10.0, 350.0, 15.0), vec![10.0, 350.0]);
    }

    #[test]
    fn turn_headings_short_wrap_reverse_direction() {
        // Python reference: get_turn_headings(350, 10, stepsize=15) == [350.0, 10.0]
        assert_eq!(turn_headings(350.0, 10.0, 15.0), vec![350.0, 10.0]);
    }

    #[test]
    fn turn_headings_simple_clockwise_step() {
        // Python reference: get_turn_headings(0, 90, stepsize=30) == [0.0, 45.0, 90.0]
        assert_eq!(turn_headings(0.0, 90.0, 30.0), vec![0.0, 45.0, 90.0]);
    }

    #[test]
    fn lat_lon_to_pixel_of_the_center_point_is_the_image_center() {
        let px = lat_lon_to_pixel(10.0, 20.0, (10.0, 20.0), 5, (200, 200));
        assert_close(px.0, 100.0, 1e-6);
        assert_close(px.1, 100.0, 1e-6);
    }

    #[test]
    fn lat_lon_to_pixel_matches_hand_computed_offset_at_zoom_zero() {
        // world_size at zoom 0 is 256px; lon=90 vs center lon=0 is a quarter
        // of the world east, i.e. +64px from center at zoom 0.
        let px = lat_lon_to_pixel(0.0, 90.0, (0.0, 0.0), 0, (256, 256));
        assert_close(px.0, 192.0, 1e-6);
        assert_close(px.1, 128.0, 1e-6);
    }

    #[test]
    fn bbox_center_zoom_centers_on_a_symmetric_bbox() {
        let points = [(0.0, -1.0), (0.0, 1.0)];
        let (center, zoom) = bbox_center_zoom(&points, 200, 200);
        assert_close(center.0, 0.0, 1e-6);
        assert_close(center.1, 0.0, 1e-6);

        // The chosen zoom must fit the bbox within the requested size...
        let world_size = TILE_SIZE * 2f64.powi(zoom as i32);
        let px_width = (lon_to_x(1.0) - lon_to_x(-1.0)) * world_size;
        assert!(px_width <= 200.0);

        // ...and be the largest such zoom (one more level would overflow it,
        // unless we're already capped at MAX_ZOOM).
        let next_world_size = TILE_SIZE * 2f64.powi(zoom as i32 + 1);
        let next_px_width = (lon_to_x(1.0) - lon_to_x(-1.0)) * next_world_size;
        assert!(next_px_width > 200.0 || zoom == MAX_ZOOM);
    }

    #[test]
    fn bbox_center_zoom_never_exceeds_max_zoom() {
        let points = [(0.0, 0.0), (0.000001, 0.000001)];
        let (_, zoom) = bbox_center_zoom(&points, 4000, 4000);
        assert!(zoom <= MAX_ZOOM);
    }
}
