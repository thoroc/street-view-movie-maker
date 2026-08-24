use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl MapCorner {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "top-left" => Ok(MapCorner::TopLeft),
            "top-right" => Ok(MapCorner::TopRight),
            "bottom-left" => Ok(MapCorner::BottomLeft),
            "bottom-right" => Ok(MapCorner::BottomRight),
            other => Err(format!(
                "invalid --map-corner {other:?} (expected one of: top-left, top-right, bottom-left, bottom-right)"
            )),
        }
    }
}

/// The inset's on-frame footprint and margin, as percentages of the frame's
/// shorter dimension — keeps the inset proportionally consistent across
/// 16:9, portrait, and square outputs rather than a fixed pixel size that
/// could occlude or crop on unusual aspect ratios.
pub const INSET_FOOTPRINT_PERCENT: f64 = 0.25;
pub const INSET_MARGIN_PERCENT: f64 = 0.03;

const MARKER_RADIUS_PX: f64 = 6.0;
const MARKER_COLOR: Rgba<u8> = Rgba([230, 30, 30, 255]);

/// Fill color for the corners exposed when rotating the pre-rotation crop
/// (see `safe_rotation_source_size`) — only visible if that crop got
/// clamped smaller than the ideal safe size near the fetched map's edges.
/// A light neutral gray, close to Google's own basemap background.
const ROTATION_FILL_COLOR: Rgba<u8> = Rgba([225, 225, 220, 255]);

/// Computes the inset's on-frame `(x, y, width, height)` rectangle for a
/// given frame size, corner, footprint, and margin.
pub fn corner_rect(
    frame_size: (u32, u32),
    corner: MapCorner,
    footprint_percent: f64,
    margin_percent: f64,
) -> (u32, u32, u32, u32) {
    let shorter = f64::from(frame_size.0.min(frame_size.1));
    let footprint = (shorter * footprint_percent).round() as u32;
    let margin = (shorter * margin_percent).round() as u32;
    let (x, y) = match corner {
        MapCorner::TopLeft => (margin, margin),
        MapCorner::TopRight => (frame_size.0.saturating_sub(footprint + margin), margin),
        MapCorner::BottomLeft => (margin, frame_size.1.saturating_sub(footprint + margin)),
        MapCorner::BottomRight => (
            frame_size.0.saturating_sub(footprint + margin),
            frame_size.1.saturating_sub(footprint + margin),
        ),
    };
    (x, y, footprint, footprint)
}

/// Computes the `(x, y, width, height)` window to crop out of a `base_size`
/// image so it's centered on `center_px` — clamped so the crop never runs
/// off the base image's edges, and clamped to `base_size` if `crop_size` is
/// larger than the base image itself. Used to pan/crop a small local-area
/// window around the current position out of one larger, whole-route base
/// map, rather than re-fetching a re-centered map per frame.
fn crop_window(
    base_size: (u32, u32),
    center_px: (f64, f64),
    crop_size: (u32, u32),
) -> (u32, u32, u32, u32) {
    let crop_w = crop_size.0.min(base_size.0);
    let crop_h = crop_size.1.min(base_size.1);
    let max_x = base_size.0 - crop_w;
    let max_y = base_size.1 - crop_h;
    let x = (center_px.0 - f64::from(crop_w) / 2.0)
        .round()
        .clamp(0.0, f64::from(max_x)) as u32;
    let y = (center_px.1 - f64::from(crop_h) / 2.0)
        .round()
        .clamp(0.0, f64::from(max_y)) as u32;
    (x, y, crop_w, crop_h)
}

/// The smallest square side that, once rotated by any angle about its own
/// center, still fully covers a `crop_size` rectangle cropped from that same
/// center — a rotated shape's farthest-from-center point (here, `crop_size`'s
/// corner) stays the same distance from center regardless of rotation angle,
/// so a source square with at least that diagonal as its side guarantees no
/// exposed (fill-color) corners in the final crop after rotation.
fn safe_rotation_source_size(crop_size: (u32, u32)) -> u32 {
    let diagonal = (f64::from(crop_size.0).powi(2) + f64::from(crop_size.1).powi(2)).sqrt();
    diagonal.ceil() as u32
}

/// Hand-rolled filled-circle draw — a single primitive doesn't need a
/// drawing-library dependency like `imageproc`.
pub fn draw_marker(image: &mut RgbaImage, center: (f64, f64), radius: f64, color: Rgba<u8>) {
    let (cx, cy) = center;
    let r2 = radius * radius;
    let min_x = (cx - radius).floor().max(0.0) as i64;
    let max_x = ((cx + radius).ceil() as i64).min(i64::from(image.width()) - 1);
    let min_y = (cy - radius).floor().max(0.0) as i64;
    let max_y = ((cy + radius).ceil() as i64).min(i64::from(image.height()) - 1);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            if dx * dx + dy * dy <= r2 {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CompositingError {
    #[error("failed to decode {path}: {source}")]
    Decode {
        path: String,
        #[source]
        source: image::ImageError,
    },
    #[error("failed to write {path}: {source}")]
    Encode {
        path: String,
        #[source]
        source: image::ImageError,
    },
}

#[derive(Debug, Clone)]
pub struct CompositeParams {
    pub corner: MapCorner,
    pub margin_percent: f64,
    pub footprint_percent: f64,
    pub map_center: (f64, f64),
    pub map_zoom: u32,
    pub map_size: (u32, u32),
    /// Size of the local-area window panned/cropped out of the base map
    /// image, centered on the current frame's position (see `crop_window`).
    /// Backs `--map-size`.
    pub crop_size: (u32, u32),
}

fn composite_frame(
    frame_path: &Path,
    base_map: &RgbaImage,
    point: (f64, f64, f64),
    params: &CompositeParams,
    dest_path: &Path,
) -> Result<(), CompositingError> {
    let (lat, lon, heading) = point;
    let frame = image::open(frame_path)
        .map_err(|source| CompositingError::Decode {
            path: frame_path.display().to_string(),
            source,
        })?
        .to_rgba8();

    let mut map = base_map.clone();
    let marker_px = crate::geo::lat_lon_to_pixel(
        lat,
        lon,
        params.map_center,
        params.map_zoom,
        params.map_size,
    );
    draw_marker(&mut map, marker_px, MARKER_RADIUS_PX, MARKER_COLOR);

    // Crop a source window big enough that rotating it about its center
    // can't expose fill-colored corners in the final crop_size crop below,
    // then rotate it so the current direction of travel points to the top
    // of the inset ("track-up", like a car GPS display) instead of the map
    // staying fixed north-up.
    let source_side = safe_rotation_source_size(params.crop_size)
        .min(map.width())
        .min(map.height());
    let (src_x, src_y, src_w, src_h) =
        crop_window(map.dimensions(), marker_px, (source_side, source_side));
    let source = image::imageops::crop_imm(&map, src_x, src_y, src_w, src_h).to_image();
    let rotated = imageproc::geometric_transformations::rotate_about_center(
        &source,
        (heading as f32).to_radians(),
        imageproc::geometric_transformations::Interpolation::Bilinear,
        ROTATION_FILL_COLOR,
    );

    // Rotating about the source crop's center keeps the marker (which was
    // centered in that crop, per `crop_window`) at the rotated image's
    // center too, so the final crop just needs to be centered on it.
    let rotated_center = (
        f64::from(rotated.width()) / 2.0,
        f64::from(rotated.height()) / 2.0,
    );
    let (crop_x, crop_y, crop_w, crop_h) =
        crop_window(rotated.dimensions(), rotated_center, params.crop_size);
    let cropped = image::imageops::crop_imm(&rotated, crop_x, crop_y, crop_w, crop_h).to_image();

    let (x, y, w, h) = corner_rect(
        frame.dimensions(),
        params.corner,
        params.footprint_percent,
        params.margin_percent,
    );
    let resized = image::imageops::resize(&cropped, w, h, image::imageops::FilterType::Lanczos3);

    let mut frame = frame;
    image::imageops::overlay(&mut frame, &resized, i64::from(x), i64::from(y));

    image::DynamicImage::ImageRgba8(frame)
        .to_rgb8()
        .save(dest_path)
        .map_err(|source| CompositingError::Encode {
            path: dest_path.display().to_string(),
            source,
        })
}

/// Composites the inset map (with a per-frame marker) onto every frame,
/// reusing `streetview::run_bounded`'s bounded-concurrency pattern since
/// this runs over potentially thousands of frames. `composited/` is always
/// fully rebuilt from `frames` every run — no resume/skip logic, matching
/// the existing `lineup/` convention, since compositing is local-only (no
/// API spend to save by skipping it).
pub async fn composite_all(
    frames: Vec<(usize, PathBuf, (f64, f64, f64))>,
    base_map_path: PathBuf,
    dest_dir: PathBuf,
    params: CompositeParams,
    concurrency: usize,
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let base_map = image::open(&base_map_path)
        .map_err(|e| format!("failed to decode {}: {e}", base_map_path.display()))?
        .to_rgba8();
    let base_map = std::sync::Arc::new(base_map);
    let params = std::sync::Arc::new(params);

    let results = crate::streetview::run_bounded(frames, concurrency.max(1), {
        let base_map = base_map.clone();
        let params = params.clone();
        let dest_dir = dest_dir.clone();
        move |(i, frame_path, point)| {
            let base_map = base_map.clone();
            let params = params.clone();
            let dest_path = dest_dir.join(format!("frame{i}.jpg"));
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    composite_frame(&frame_path, &base_map, point, &params, &dest_path)
                        .map(|()| dest_path)
                })
                .await;
                (i, result)
            }
        }
    })
    .await;

    let mut out: Vec<PathBuf> = Vec::with_capacity(results.len());
    for (i, joined) in results {
        let path = joined.map_err(|e| format!("compositing task panicked: {e}"))?;
        out.push(path.map_err(|e| format!("compositing frame {i} failed: {e}"))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_all_four_corners() {
        assert_eq!(MapCorner::parse("top-left").unwrap(), MapCorner::TopLeft);
        assert_eq!(MapCorner::parse("top-right").unwrap(), MapCorner::TopRight);
        assert_eq!(
            MapCorner::parse("bottom-left").unwrap(),
            MapCorner::BottomLeft
        );
        assert_eq!(
            MapCorner::parse("bottom-right").unwrap(),
            MapCorner::BottomRight
        );
    }

    #[test]
    fn parse_rejects_unknown_corner() {
        assert!(MapCorner::parse("middle").is_err());
    }

    #[test]
    fn corner_rect_places_bottom_right_inset_flush_with_margin() {
        let (x, y, w, h) = corner_rect((1000, 500), MapCorner::BottomRight, 0.2, 0.05);
        // shorter side is 500: footprint = 100, margin = 25.
        assert_eq!((w, h), (100, 100));
        assert_eq!(x, 1000 - 100 - 25);
        assert_eq!(y, 500 - 100 - 25);
    }

    #[test]
    fn corner_rect_places_top_left_at_the_margin() {
        let (x, y, w, h) = corner_rect((1000, 500), MapCorner::TopLeft, 0.2, 0.05);
        assert_eq!((x, y), (25, 25));
        assert_eq!((w, h), (100, 100));
    }

    #[test]
    fn corner_rect_footprint_is_consistent_across_aspect_ratios() {
        let landscape = corner_rect((1920, 1080), MapCorner::BottomRight, 0.25, 0.03);
        let portrait = corner_rect((1080, 1920), MapCorner::BottomRight, 0.25, 0.03);
        assert_eq!(landscape.2, portrait.2);
        assert_eq!(landscape.3, portrait.3);
    }

    #[test]
    fn crop_window_centers_on_the_point_away_from_edges() {
        let (x, y, w, h) = crop_window((640, 640), (320.0, 320.0), (200, 200));
        assert_eq!((w, h), (200, 200));
        assert_eq!(x, 220); // 320 - 200/2
        assert_eq!(y, 220);
    }

    #[test]
    fn crop_window_clamps_to_the_base_image_near_an_edge() {
        let (x, y, w, h) = crop_window((640, 640), (5.0, 635.0), (200, 200));
        assert_eq!((w, h), (200, 200));
        assert_eq!(x, 0); // would be negative uncentered; clamped to 0
        assert_eq!(y, 440); // would overflow past 640; clamped to 640-200
    }

    #[test]
    fn crop_window_clamps_crop_size_to_the_base_image_size() {
        let (x, y, w, h) = crop_window((200, 200), (100.0, 100.0), (640, 640));
        assert_eq!((x, y), (0, 0));
        assert_eq!((w, h), (200, 200));
    }

    #[test]
    fn safe_rotation_source_size_is_the_crop_diagonal() {
        // 200x200's diagonal is 200*sqrt(2) ~= 282.84, rounded up.
        assert_eq!(safe_rotation_source_size((200, 200)), 283);
    }

    #[test]
    fn safe_rotation_source_size_handles_non_square_crops() {
        let side = safe_rotation_source_size((100, 200));
        let expected = (100f64.powi(2) + 200f64.powi(2)).sqrt().ceil() as u32;
        assert_eq!(side, expected);
    }

    #[test]
    fn draw_marker_paints_a_filled_circle() {
        let mut image = RgbaImage::from_pixel(50, 50, Rgba([0, 0, 0, 255]));
        draw_marker(&mut image, (25.0, 25.0), 5.0, Rgba([255, 0, 0, 255]));
        assert_eq!(*image.get_pixel(25, 25), Rgba([255, 0, 0, 255]));
        assert_eq!(*image.get_pixel(0, 0), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn draw_marker_clamps_to_image_bounds_without_panicking() {
        let mut image = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255]));
        draw_marker(&mut image, (0.0, 0.0), 5.0, Rgba([255, 0, 0, 255]));
        draw_marker(&mut image, (19.0, 19.0), 5.0, Rgba([255, 0, 0, 255]));
    }
}
