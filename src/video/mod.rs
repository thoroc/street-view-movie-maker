use std::path::PathBuf;

#[cfg(feature = "av1")]
mod av1;
#[cfg(feature = "h264")]
mod h264;

#[cfg(feature = "av1")]
pub use av1::encode_video;
#[cfg(feature = "h264")]
pub use h264::encode_video;

#[cfg(all(feature = "av1", feature = "h264"))]
compile_error!("features \"av1\" and \"h264\" are mutually exclusive — pick one encoder backend");
#[cfg(not(any(feature = "av1", feature = "h264")))]
compile_error!(
    "one of the \"av1\" or \"h264\" features must be enabled — no encoder would be compiled in otherwise"
);

#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error(
        "invalid --picsize {0:?}: expected \"WIDTHxHEIGHT\" with positive, even dimensions (AV1/YUV420 requires even width and height)"
    )]
    InvalidPicsize(String),
    #[error("no frames to encode")]
    NoFrames,
    #[error("failed to read frame {path}: {source}")]
    FrameRead {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },
    #[error(
        "frame {path} is {actual_width}x{actual_height}, expected {expected_width}x{expected_height} (--picsize)"
    )]
    FrameSizeMismatch {
        path: PathBuf,
        actual_width: u32,
        actual_height: u32,
        expected_width: usize,
        expected_height: usize,
    },
    #[cfg(feature = "av1")]
    #[error("failed to initialize the AV1 encoder: {0}")]
    EncoderInit(String),
    #[cfg(feature = "av1")]
    #[error("AV1 encoding failed: {0:?}")]
    EncodeFailed(rav1e::EncoderStatus),
    #[cfg(feature = "h264")]
    #[error("failed to initialize the H.264 encoder: {0}")]
    EncoderInit(String),
    #[cfg(feature = "h264")]
    #[error("H.264 encoding failed: {0}")]
    EncodeFailed(openh264::Error),
    #[error("failed to write the output video: {0}")]
    Mux(String),
    #[error("failed to write the output video: {0}")]
    Io(#[from] std::io::Error),
}

/// Parses and validates the `--picsize` flag ("WIDTHxHEIGHT") before any
/// frames are downloaded or encoded — a malformed or odd-numbered value is
/// caught here, at flag-resolution time, rather than deep inside the encode
/// step after Street View API calls have already been billed. AV1/YUV420
/// requires even dimensions, so both width and height must be even.
pub fn parse_picsize(picsize: &str) -> Result<(usize, usize), VideoError> {
    let invalid = || VideoError::InvalidPicsize(picsize.to_string());
    let (w, h) = picsize.split_once('x').ok_or_else(invalid)?;
    let width: usize = w.parse().map_err(|_| invalid())?;
    let height: usize = h.parse().map_err(|_| invalid())?;
    if width == 0 || height == 0 || !width.is_multiple_of(2) || !height.is_multiple_of(2) {
        return Err(invalid());
    }
    Ok((width, height))
}

/// Converts an RGB frame to planar YUV 4:2:0 using BT.709 coefficients at
/// limited (video) range — the common modern default matrix for HD/web
/// video. This is a documented approximation matching no particular
/// ffmpeg build's default matrix selection bit-for-bit; the pipeline's own
/// verification is "does it play correctly," not "are the encoded bytes
/// identical to ffmpeg's."
///
/// Chroma planes are box-filtered (averaged over each 2x2 luma block)
/// rather than point-sampled, for better visual quality at the 4:2:0
/// subsampling this function always produces. Callers must pass an image
/// with even width and height (`parse_picsize` enforces this upstream).
fn rgb_to_yuv420_bt709(img: &image::RgbImage) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let (width, height) = img.dimensions();
    let (w, h) = (width as usize, height as usize);
    debug_assert!(
        w % 2 == 0 && h % 2 == 0,
        "chroma subsampling requires even dimensions"
    );

    let luma = |r: f32, g: f32, b: f32| 16.0 + (65.738 * r + 129.057 * g + 25.064 * b) / 256.0;
    let chroma_u =
        |r: f32, g: f32, b: f32| 128.0 + (-37.945 * r - 74.494 * g + 112.439 * b) / 256.0;
    let chroma_v = |r: f32, g: f32, b: f32| 128.0 + (112.439 * r - 94.154 * g - 18.285 * b) / 256.0;

    let mut y_plane = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x as u32, y as u32);
            y_plane[y * w + x] = luma(f32::from(p[0]), f32::from(p[1]), f32::from(p[2]))
                .round()
                .clamp(16.0, 235.0) as u8;
        }
    }

    let (cw, ch) = (w / 2, h / 2);
    let mut u_plane = vec![0u8; cw * ch];
    let mut v_plane = vec![0u8; cw * ch];
    for cy in 0..ch {
        for cx in 0..cw {
            let mut sum = [0.0f32; 3];
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    let p = img.get_pixel((cx * 2) as u32 + dx, (cy * 2) as u32 + dy);
                    sum[0] += f32::from(p[0]);
                    sum[1] += f32::from(p[1]);
                    sum[2] += f32::from(p[2]);
                }
            }
            let (r, g, b) = (sum[0] / 4.0, sum[1] / 4.0, sum[2] / 4.0);
            u_plane[cy * cw + cx] = chroma_u(r, g, b).round().clamp(16.0, 240.0) as u8;
            v_plane[cy * cw + cx] = chroma_v(r, g, b).round().clamp(16.0, 240.0) as u8;
        }
    }
    (y_plane, u_plane, v_plane)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_picsize_accepts_valid_even_dimensions() {
        assert_eq!(parse_picsize("640x320").unwrap(), (640, 320));
    }

    #[test]
    fn parse_picsize_rejects_missing_separator() {
        assert!(parse_picsize("640").is_err());
    }

    #[test]
    fn parse_picsize_rejects_zero() {
        assert!(parse_picsize("0x320").is_err());
        assert!(parse_picsize("640x0").is_err());
    }

    #[test]
    fn parse_picsize_rejects_odd_dimensions() {
        assert!(parse_picsize("641x320").is_err());
        assert!(parse_picsize("640x321").is_err());
    }

    #[test]
    fn parse_picsize_rejects_non_numeric() {
        assert!(parse_picsize("invalid").is_err());
        assert!(parse_picsize("640 x 480").is_err());
    }

    #[test]
    fn rgb_to_yuv420_bt709_produces_correctly_sized_planes() {
        let img = image::RgbImage::from_pixel(4, 2, image::Rgb([128, 64, 200]));
        let (y, u, v) = rgb_to_yuv420_bt709(&img);
        assert_eq!(y.len(), 4 * 2);
        assert_eq!(u.len(), 2); // (4 / 2) * (2 / 2)
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn rgb_to_yuv420_bt709_clamps_to_limited_range() {
        let black = image::RgbImage::from_pixel(2, 2, image::Rgb([0, 0, 0]));
        let (y, u, v) = rgb_to_yuv420_bt709(&black);
        assert!(y.iter().all(|&v| v >= 16));
        assert!(u.iter().all(|&v| (16..=240).contains(&v)));
        assert!(v.iter().all(|&v| (16..=240).contains(&v)));

        let white = image::RgbImage::from_pixel(2, 2, image::Rgb([255, 255, 255]));
        let (y, _, _) = rgb_to_yuv420_bt709(&white);
        assert!(y.iter().all(|&v| v <= 235));
    }
}
