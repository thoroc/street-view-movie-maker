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
pub struct InsetMapParams {
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

/// A maneuver already matched to its nearest frame index (see
/// `match_maneuvers_to_frames`), for cheap forward-scanning inside the
/// per-frame compositing loop instead of a haversine search per frame.
#[derive(Debug, Clone)]
pub struct MatchedManeuver {
    pub frame_index: usize,
    pub maneuver: crate::directions::Maneuver,
}

#[derive(Debug, Clone)]
pub struct TurnSignParams {
    pub corner: MapCorner,
    /// Sorted by `frame_index` — see `match_maneuvers_to_frames`.
    pub matched: Vec<MatchedManeuver>,
    pub lead_frames: usize,
}

/// Matches each maneuver to its nearest frame (haversine, against each
/// frame's own position) once up front. Independent of
/// `itinerary::filter_maneuvers_by_proximity`, which runs earlier against
/// the post-dedupe point list, before turn-frame insertion — this runs
/// against the actual final frame sequence, which can differ in count and
/// order after lineup/turn-frame processing.
pub fn match_maneuvers_to_frames(
    maneuvers: &[crate::directions::Maneuver],
    frames: &[(usize, PathBuf, (f64, f64, f64))],
) -> Vec<MatchedManeuver> {
    let mut matched: Vec<MatchedManeuver> = maneuvers
        .iter()
        .filter_map(|maneuver| {
            frames
                .iter()
                .min_by(|a, b| {
                    let da = crate::geo::haversine_meters(maneuver.at, (a.2.0, a.2.1));
                    let db = crate::geo::haversine_meters(maneuver.at, (b.2.0, b.2.1));
                    da.total_cmp(&db)
                })
                .map(|(frame_index, _, _)| MatchedManeuver {
                    frame_index: *frame_index,
                    maneuver: maneuver.clone(),
                })
        })
        .collect();
    matched.sort_by_key(|m| m.frame_index);
    matched
}

/// The nearest upcoming maneuver for `frame_index`, if one's matched frame
/// falls within `[frame_index, frame_index + lead_frames]`. Approaching
/// from the frame's side (adding `lead_frames` rather than subtracting it
/// from the target) means a maneuver near the route's start can't underflow
/// the window. `matched` must be sorted by `frame_index`.
pub fn upcoming_maneuver_for_frame(
    matched: &[MatchedManeuver],
    frame_index: usize,
    lead_frames: usize,
) -> Option<&crate::directions::Maneuver> {
    let window_end = frame_index.saturating_add(lead_frames);
    matched
        .iter()
        .find(|m| m.frame_index >= frame_index && m.frame_index <= window_end)
        .map(|m| &m.maneuver)
}

#[derive(Debug, Clone, Default)]
pub struct CompositeParams {
    pub inset_map: Option<InsetMapParams>,
    pub turn_sign: Option<TurnSignParams>,
}

const SIGN_BACKGROUND_COLOR: Rgba<u8> = Rgba([20, 90, 40, 235]);
const SIGN_GLYPH_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]);
const SIGN_HEIGHT_PERCENT: f64 = 0.12;
const SIGN_GAP_PERCENT: f64 = 0.015;
/// The sign's direction glyph sits in a square area this many multiples of
/// the sign's height wide, on the left; the road name fills the rest.
const GLYPH_AREA_MULTIPLE: u32 = 1;
/// Text scale as a fraction of the sign's height.
const TEXT_HEIGHT_PERCENT: f32 = 0.55;

/// Signika (SIL OFL 1.1, https://github.com/googlefonts/Signika), chosen for
/// its resemblance to the DIN 1451-style lettering used on road signage
/// across much of continental Europe — see
/// `.context/plans/2026-08-23-add-turn-ahead-road-sign-overlay.md`'s
/// Risk/cost section. `assets/fonts/Signika-OFL.txt` carries the license.
/// Baked into the binary via `include_bytes!` rather than loaded from disk
/// at runtime, matching this project's single-self-contained-binary
/// philosophy (no external asset the binary has to locate alongside it).
static SIGN_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/Signika[GRAD,wght].ttf");

fn sign_font() -> &'static ab_glyph::FontRef<'static> {
    static FONT: std::sync::LazyLock<ab_glyph::FontRef<'static>> = std::sync::LazyLock::new(|| {
        ab_glyph::FontRef::try_from_slice(SIGN_FONT_BYTES).expect("bundled Signika font is valid")
    });
    &FONT
}

/// Draws the turn-ahead sign's background plate, direction glyph, and road
/// name for `maneuver` onto `frame`, in `corner` — stacked directly above
/// `map_rect` (the inset map's on-frame rect) when present, per the layout
/// decision in the plan, or standalone in that corner otherwise.
fn draw_turn_sign(
    frame: &mut RgbaImage,
    maneuver: &crate::directions::Maneuver,
    corner: MapCorner,
    map_rect: Option<(u32, u32, u32, u32)>,
) {
    let (frame_w, frame_h) = frame.dimensions();
    let shorter = f64::from(frame_w.min(frame_h));
    let sign_h = ((shorter * SIGN_HEIGHT_PERCENT).round() as u32).max(1);
    let sign_w = sign_h * 4;

    let (anchor_x, anchor_y, anchor_w, anchor_h) = map_rect.unwrap_or_else(|| {
        corner_rect(
            frame.dimensions(),
            corner,
            INSET_FOOTPRINT_PERCENT,
            INSET_MARGIN_PERCENT,
        )
    });
    let gap = (shorter * SIGN_GAP_PERCENT).round() as i64;

    // Stack on whichever side of the anchor has room to spare: above it for
    // a bottom corner, below it for a top corner. Fixing this to "always
    // above" would push the sign off the top of the frame for
    // --map-corner top-left/top-right (found via a real end-to-end run,
    // not caught by unit tests using only bottom-corner fixtures).
    let stack_above = matches!(corner, MapCorner::BottomLeft | MapCorner::BottomRight);
    let y = if stack_above {
        i64::from(anchor_y) - gap - i64::from(sign_h)
    } else {
        i64::from(anchor_y) + i64::from(anchor_h) + gap
    };
    let max_y = (i64::from(frame_h) - i64::from(sign_h)).max(0);
    let y = y.clamp(0, max_y);

    // Centered horizontally over whatever it's anchored to (the map's rect,
    // or the same footprint the map would occupy), then clamped so a sign
    // wider than that footprint never runs off either frame edge (the same
    // real-run finding as above).
    let x = i64::from(anchor_x) + (i64::from(anchor_w) - i64::from(sign_w)) / 2;
    let max_x = (i64::from(frame_w) - i64::from(sign_w)).max(0);
    let x = x.clamp(0, max_x);

    let rect = imageproc::rect::Rect::at(x as i32, y as i32).of_size(sign_w, sign_h);
    imageproc::drawing::draw_filled_rect_mut(frame, rect, SIGN_BACKGROUND_COLOR);

    let glyph_area_w = sign_h * GLYPH_AREA_MULTIPLE;
    draw_direction_glyph(frame, maneuver.direction, x, y, glyph_area_w, sign_h);

    if let Some(road_name) = &maneuver.road_name {
        let scale = ab_glyph::PxScale::from(sign_h as f32 * TEXT_HEIGHT_PERCENT);
        let font = sign_font();
        let (_, text_h) = imageproc::drawing::text_size(scale, font, road_name);
        let text_x = (x + i64::from(glyph_area_w)) as i32;
        let text_y = (y + (i64::from(sign_h) - i64::from(text_h)) / 2) as i32;
        imageproc::drawing::draw_text_mut(
            frame,
            SIGN_GLYPH_COLOR,
            text_x,
            text_y,
            scale,
            font,
            road_name,
        );
    }
}

/// A plain triangular arrow (left/right) or chevron (straight-on/other),
/// drawn as a shape rather than a text glyph — always available regardless
/// of the road-name font.
fn draw_direction_glyph(
    frame: &mut RgbaImage,
    direction: crate::directions::TurnDirection,
    area_x: i64,
    area_y: i64,
    area_w: u32,
    area_h: u32,
) {
    use crate::directions::TurnDirection;
    use imageproc::point::Point;

    let cx = area_x + i64::from(area_w) / 2;
    let cy = area_y + i64::from(area_h) / 2;
    let arm = (i64::from(area_h) / 3).max(1);
    let pt = |x: i64, y: i64| Point::new(x as i32, y as i32);

    let points = match direction {
        TurnDirection::Left => vec![
            pt(cx - arm, cy),
            pt(cx + arm / 2, cy - arm),
            pt(cx + arm / 2, cy + arm),
        ],
        TurnDirection::Right => vec![
            pt(cx + arm, cy),
            pt(cx - arm / 2, cy - arm),
            pt(cx - arm / 2, cy + arm),
        ],
        TurnDirection::StraightOn | TurnDirection::Other => vec![
            pt(cx, cy - arm),
            pt(cx - arm, cy + arm / 2),
            pt(cx + arm, cy + arm / 2),
        ],
    };
    imageproc::drawing::draw_polygon_mut(frame, &points, SIGN_GLYPH_COLOR);
}

fn composite_frame(
    frame_path: &Path,
    base_map: Option<&RgbaImage>,
    frame_index: usize,
    point: (f64, f64, f64),
    params: &CompositeParams,
    dest_path: &Path,
) -> Result<(), CompositingError> {
    let (lat, lon, heading) = point;
    let mut frame = image::open(frame_path)
        .map_err(|source| CompositingError::Decode {
            path: frame_path.display().to_string(),
            source,
        })?
        .to_rgba8();

    let mut map_rect: Option<(u32, u32, u32, u32)> = None;
    if let (Some(inset), Some(base_map)) = (&params.inset_map, base_map) {
        let mut map = base_map.clone();
        let marker_px = crate::geo::lat_lon_to_pixel(
            lat,
            lon,
            inset.map_center,
            inset.map_zoom,
            inset.map_size,
        );
        draw_marker(&mut map, marker_px, MARKER_RADIUS_PX, MARKER_COLOR);

        // Crop a source window big enough that rotating it about its center
        // can't expose fill-colored corners in the final crop_size crop
        // below, then rotate it so the current direction of travel points
        // to the top of the inset ("track-up", like a car GPS display)
        // instead of the map staying fixed north-up.
        let source_side = safe_rotation_source_size(inset.crop_size)
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

        // Rotating about the source crop's center keeps the marker (which
        // was centered in that crop, per `crop_window`) at the rotated
        // image's center too, so the final crop just needs to be centered
        // on it.
        let rotated_center = (
            f64::from(rotated.width()) / 2.0,
            f64::from(rotated.height()) / 2.0,
        );
        let (crop_x, crop_y, crop_w, crop_h) =
            crop_window(rotated.dimensions(), rotated_center, inset.crop_size);
        let cropped =
            image::imageops::crop_imm(&rotated, crop_x, crop_y, crop_w, crop_h).to_image();

        let rect = corner_rect(
            frame.dimensions(),
            inset.corner,
            inset.footprint_percent,
            inset.margin_percent,
        );
        let (x, y, w, h) = rect;
        let resized =
            image::imageops::resize(&cropped, w, h, image::imageops::FilterType::Lanczos3);
        image::imageops::overlay(&mut frame, &resized, i64::from(x), i64::from(y));
        map_rect = Some(rect);
    }

    if let Some(turn_sign) = &params.turn_sign
        && let Some(maneuver) =
            upcoming_maneuver_for_frame(&turn_sign.matched, frame_index, turn_sign.lead_frames)
    {
        draw_turn_sign(&mut frame, maneuver, turn_sign.corner, map_rect);
    }

    image::DynamicImage::ImageRgba8(frame)
        .to_rgb8()
        .save(dest_path)
        .map_err(|source| CompositingError::Encode {
            path: dest_path.display().to_string(),
            source,
        })
}

/// Composites the inset map and/or turn-ahead signs onto every frame,
/// reusing `streetview::run_bounded`'s bounded-concurrency pattern since
/// this runs over potentially thousands of frames. `composited/` is always
/// fully rebuilt from `frames` every run — no resume/skip logic, matching
/// the existing `lineup/` convention, since compositing is local-only (no
/// API spend to save by skipping it). `base_map_path` is required exactly
/// when `params.inset_map` is `Some` — the two are decoupled (2026-08-25
/// reassessment) so turn-signs alone (`--show-turn-signs --hide-map`) still
/// composite.
pub async fn composite_all(
    frames: Vec<(usize, PathBuf, (f64, f64, f64))>,
    base_map_path: Option<PathBuf>,
    dest_dir: PathBuf,
    params: CompositeParams,
    concurrency: usize,
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let base_map = match base_map_path {
        Some(path) => Some(
            image::open(&path)
                .map_err(|e| format!("failed to decode {}: {e}", path.display()))?
                .to_rgba8(),
        ),
        None => None,
    };
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
                    composite_frame(
                        &frame_path,
                        base_map.as_ref().as_ref(),
                        i,
                        point,
                        &params,
                        &dest_path,
                    )
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

    fn maneuver(at: (f64, f64)) -> crate::directions::Maneuver {
        crate::directions::Maneuver {
            at,
            direction: crate::directions::TurnDirection::Left,
            road_name: None,
        }
    }

    fn frame(index: usize, point: (f64, f64, f64)) -> (usize, PathBuf, (f64, f64, f64)) {
        (index, PathBuf::from(format!("frame{index}.jpg")), point)
    }

    #[test]
    fn match_maneuvers_to_frames_picks_the_nearest_frame() {
        let frames = vec![
            frame(0, (0.0, 0.0, 0.0)),
            frame(1, (0.0, 1.0, 0.0)),
            frame(2, (0.0, 2.0, 0.0)),
        ];
        let maneuvers = vec![maneuver((0.0, 1.9))];
        let matched = match_maneuvers_to_frames(&maneuvers, &frames);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].frame_index, 2);
    }

    #[test]
    fn match_maneuvers_to_frames_sorts_by_frame_index() {
        let frames = vec![
            frame(0, (0.0, 0.0, 0.0)),
            frame(1, (0.0, 1.0, 0.0)),
            frame(2, (0.0, 2.0, 0.0)),
        ];
        // Second maneuver matches the earlier frame, first matches the later
        // one — the matched list should still come out sorted by index.
        let maneuvers = vec![maneuver((0.0, 2.0)), maneuver((0.0, 0.0))];
        let matched = match_maneuvers_to_frames(&maneuvers, &frames);
        assert_eq!(
            matched.iter().map(|m| m.frame_index).collect::<Vec<_>>(),
            vec![0, 2]
        );
    }

    #[test]
    fn upcoming_maneuver_for_frame_finds_a_maneuver_within_the_lead_window() {
        let matched = vec![MatchedManeuver {
            frame_index: 10,
            maneuver: maneuver((0.0, 0.0)),
        }];
        assert!(upcoming_maneuver_for_frame(&matched, 5, 5).is_some());
        assert!(upcoming_maneuver_for_frame(&matched, 10, 5).is_some());
        assert!(upcoming_maneuver_for_frame(&matched, 4, 5).is_none());
        assert!(upcoming_maneuver_for_frame(&matched, 11, 5).is_none());
    }

    #[test]
    fn upcoming_maneuver_for_frame_never_underflows_near_route_start() {
        let matched = vec![MatchedManeuver {
            frame_index: 2,
            maneuver: maneuver((0.0, 0.0)),
        }];
        // frame_index 0 with a lead window of 5 would require `2 - 5` if the
        // window were computed by subtracting from the target; this must
        // not panic or wrap, and 2 is within [0, 5].
        assert!(upcoming_maneuver_for_frame(&matched, 0, 5).is_some());
    }

    #[test]
    fn upcoming_maneuver_for_frame_gives_each_close_maneuver_its_own_approach() {
        let matched = vec![
            MatchedManeuver {
                frame_index: 100,
                maneuver: maneuver((0.0, 0.0)),
            },
            MatchedManeuver {
                frame_index: 105,
                maneuver: maneuver((0.0, 1.0)),
            },
        ];
        // Both are in range at frame 96; the nearer one (100) wins.
        let at_96 = upcoming_maneuver_for_frame(&matched, 96, 10).unwrap();
        assert_eq!(at_96.at, (0.0, 0.0));
        // Once frame 100 has passed, the second maneuver takes over instead
        // of being permanently shadowed by the first.
        let at_101 = upcoming_maneuver_for_frame(&matched, 101, 10).unwrap();
        assert_eq!(at_101.at, (0.0, 1.0));
    }

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

    #[test]
    fn draw_turn_sign_paints_a_background_plate_standalone() {
        let mut image = RgbaImage::from_pixel(200, 100, Rgba([0, 0, 0, 255]));
        draw_turn_sign(
            &mut image,
            &maneuver((0.0, 0.0)),
            MapCorner::BottomRight,
            None,
        );
        let (x, y, _, _) = corner_rect(
            (200, 100),
            MapCorner::BottomRight,
            INSET_FOOTPRINT_PERCENT,
            INSET_MARGIN_PERCENT,
        );
        // The sign sits just above where the (absent) map's footprint would
        // be — some pixel in that column should no longer be the plain
        // black background.
        let sample_y = y.saturating_sub(5);
        assert_ne!(*image.get_pixel(x, sample_y), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn draw_turn_sign_stacks_above_the_given_map_rect() {
        let mut image = RgbaImage::from_pixel(200, 200, Rgba([0, 0, 0, 255]));
        let map_rect = (50, 150, 80, 40);
        draw_turn_sign(
            &mut image,
            &maneuver((0.0, 0.0)),
            MapCorner::BottomRight,
            Some(map_rect),
        );
        // Directly under the map's top edge should be untouched by the sign.
        assert_eq!(*image.get_pixel(50, 151), Rgba([0, 0, 0, 255]));
        // Somewhere above the map's top edge should now have the sign's
        // background color.
        let mut found_sign_pixel = false;
        for y in 100..map_rect.1 {
            if *image.get_pixel(70, y) == SIGN_BACKGROUND_COLOR {
                found_sign_pixel = true;
                break;
            }
        }
        assert!(
            found_sign_pixel,
            "expected to find the sign above the map rect"
        );
    }

    #[test]
    fn draw_turn_sign_renders_the_road_name_when_present() {
        let mut with_name = RgbaImage::from_pixel(400, 100, Rgba([0, 0, 0, 255]));
        let mut without_name = RgbaImage::from_pixel(400, 100, Rgba([0, 0, 0, 255]));
        let named = crate::directions::Maneuver {
            at: (0.0, 0.0),
            direction: crate::directions::TurnDirection::Left,
            road_name: Some("Main St".to_string()),
        };
        draw_turn_sign(&mut with_name, &named, MapCorner::BottomRight, None);
        draw_turn_sign(
            &mut without_name,
            &maneuver((0.0, 0.0)),
            MapCorner::BottomRight,
            None,
        );
        // Somewhere in the sign's text half (right of the glyph area), the
        // two images must differ — proves the font actually drew glyphs
        // rather than silently no-op'ing.
        assert_ne!(
            with_name.as_raw(),
            without_name.as_raw(),
            "expected road-name text to change pixels within the sign"
        );
    }

    /// Regression test for a real bug found running the actual binary
    /// end-to-end: at the default 640x320 picsize with the default
    /// bottom-right map corner, the sign (wider than the map's own
    /// footprint) extended past the frame's right edge before the x-clamp
    /// was added.
    #[test]
    fn draw_turn_sign_never_overflows_the_frame_at_default_picsize() {
        let (frame_w, frame_h) = (640, 320);
        let mut image = RgbaImage::from_pixel(frame_w, frame_h, Rgba([0, 0, 0, 255]));
        let named = crate::directions::Maneuver {
            at: (0.0, 0.0),
            direction: crate::directions::TurnDirection::Right,
            road_name: Some("W 33rd St".to_string()),
        };
        let map_rect = corner_rect(
            (frame_w, frame_h),
            MapCorner::BottomRight,
            INSET_FOOTPRINT_PERCENT,
            INSET_MARGIN_PERCENT,
        );
        draw_turn_sign(&mut image, &named, MapCorner::BottomRight, Some(map_rect));
        // Nothing should have panicked (an out-of-bounds draw would have),
        // and the rightmost column must still be plain background — proof
        // the sign didn't get clipped by silently drawing past the edge.
        assert_eq!(
            *image.get_pixel(frame_w - 1, frame_h - 1),
            Rgba([0, 0, 0, 255])
        );
    }

    #[test]
    fn draw_turn_sign_stacks_below_a_top_corner_map_instead_of_off_frame() {
        let (frame_w, frame_h) = (300, 200);
        let mut image = RgbaImage::from_pixel(frame_w, frame_h, Rgba([0, 0, 0, 255]));
        let map_rect = corner_rect(
            (frame_w, frame_h),
            MapCorner::TopLeft,
            INSET_FOOTPRINT_PERCENT,
            INSET_MARGIN_PERCENT,
        );
        draw_turn_sign(
            &mut image,
            &maneuver((0.0, 0.0)),
            MapCorner::TopLeft,
            Some(map_rect),
        );
        // "Above" a top-corner map is off-frame; the sign must render
        // below it instead, not get clamped invisibly to y=0 overlapping
        // the map.
        let (_, map_y, _, map_h) = map_rect;
        let mut found_sign_pixel = false;
        for y in (map_y + map_h)..frame_h {
            if *image.get_pixel(10, y) == SIGN_BACKGROUND_COLOR {
                found_sign_pixel = true;
                break;
            }
        }
        assert!(
            found_sign_pixel,
            "expected the sign below the top-corner map, not off-frame"
        );
    }
}
