//! Street View / Maps Static API cost estimation.

/// Street View Static API pricing per Google's published rate card
/// (developers.google.com/maps/billing-and-pricing/pricing, last checked
/// 2026-08-23): $7.00 per 1,000 images, with the first 10,000/month free.
/// This estimate assumes no free-tier allowance remains this month, since
/// the CLI has no way to know how much of it you've already used — the
/// actual charge may be $0 if you're within that allowance.
pub const STREETVIEW_PRICE_PER_1000_USD: f64 = 7.00;

/// Maps Static API pricing per Google's published rate card (same source as
/// the Street View constant above, last checked 2026-08-23): $2.00 per 1,000
/// requests, with the first 10,000/month free. Only one Static Maps request
/// happens per run regardless of frame count.
pub const STATIC_MAP_PRICE_PER_1000_USD: f64 = 2.00;

pub fn estimate_download_cost_usd(image_count: usize) -> f64 {
    image_count as f64 / 1000.0 * STREETVIEW_PRICE_PER_1000_USD
}

#[cfg(test)]
mod tests {
    #[test]
    fn estimate_download_cost_usd_scales_with_image_count() {
        assert_eq!(super::estimate_download_cost_usd(0), 0.0);
        assert_eq!(super::estimate_download_cost_usd(1000), 7.0);
        assert_eq!(super::estimate_download_cost_usd(500), 3.5);
    }

    #[test]
    fn estimate_download_cost_usd_matches_a_realistic_route_size() {
        // A ~90km route (like the Marseille-airport-to-Simiane-la-Rotonde
        // example) produced ~8500 images in practice.
        let cost = super::estimate_download_cost_usd(8500);
        assert!(
            (cost - 59.5).abs() < 1e-9,
            "expected ~$59.50 for 8500 images, got ${cost}"
        );
    }
}
