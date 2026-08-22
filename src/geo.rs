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

pub fn clean_look_points(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut cleaned: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for &point in points {
        if cleaned.last() != Some(&point) {
            cleaned.push(point);
        }
    }
    cleaned
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
    fn clean_look_points_removes_consecutive_duplicates() {
        let input = vec![(1.0, 1.0), (1.0, 1.0), (2.0, 2.0), (2.0, 2.0), (3.0, 3.0)];
        let cleaned = clean_look_points(&input);
        assert_eq!(cleaned, vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]);
    }

    #[test]
    fn clean_look_points_keeps_non_consecutive_repeats() {
        let input = vec![(1.0, 1.0), (2.0, 2.0), (1.0, 1.0)];
        let cleaned = clean_look_points(&input);
        assert_eq!(cleaned, input);
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
}
