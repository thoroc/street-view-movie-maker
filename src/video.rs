use std::path::{Path, PathBuf};

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

#[cfg(feature = "av1")]
mod av1_encoder {
    use super::{VideoError, rgb_to_yuv420_bt709};
    use rav1e::prelude::*;
    use std::path::PathBuf;

    /// One encoded AV1 temporal unit (OBU stream) plus whether it's a
    /// keyframe — muxide's `write_video` needs the latter explicitly since
    /// it can't be inferred from the OBU bytes without re-parsing them.
    pub struct EncodedPacket {
        pub data: Vec<u8>,
        pub is_keyframe: bool,
    }

    fn build_context(width: usize, height: usize, fps: u32) -> Result<Context<u8>, VideoError> {
        let enc = EncoderConfig {
            width,
            height,
            time_base: Rational::new(1, u64::from(fps)),
            // rav1e has no CRF-style quality knob; `quantizer` (this is
            // EncoderConfig's own default) is the closest analog and is not
            // claimed to be visually equivalent to the previous `-crf 25`
            // ffmpeg setting — see the plan's Open Questions.
            ..Default::default()
        };
        Config::new()
            .with_encoder_config(enc)
            .new_context()
            .map_err(|e| VideoError::EncoderInit(format!("{e:?}")))
    }

    fn frame_from_rgb(ctx: &Context<u8>, img: &image::RgbImage) -> Frame<u8> {
        let width = img.width() as usize;
        let (y, u, v) = rgb_to_yuv420_bt709(img);
        let mut frame = ctx.new_frame();
        frame.planes[0].copy_from_raw_u8(&y, width, 1);
        frame.planes[1].copy_from_raw_u8(&u, width / 2, 1);
        frame.planes[2].copy_from_raw_u8(&v, width / 2, 1);
        frame
    }

    fn drain_available_packets(
        ctx: &mut Context<u8>,
        out: &mut Vec<EncodedPacket>,
    ) -> Result<(), VideoError> {
        loop {
            match ctx.receive_packet() {
                Ok(packet) => out.push(EncodedPacket {
                    data: packet.data,
                    is_keyframe: packet.frame_type == FrameType::KEY,
                }),
                // Internal step processed with nothing to emit yet, or the
                // encoder's lookahead needs another input frame before it
                // can produce one — both mean "stop draining for now," not
                // an error.
                Err(EncoderStatus::Encoded) | Err(EncoderStatus::NeedMoreData) => return Ok(()),
                Err(e) => return Err(VideoError::EncodeFailed(e)),
            }
        }
    }

    /// Encodes an ordered sequence of same-sized frame files into AV1
    /// temporal units. `frame_paths` must already be in playback order —
    /// this project's frame-numbering pass (`lineup::renumber_sequentially`)
    /// guarantees that upstream, so no directory re-scan/re-sort happens
    /// here.
    pub fn encode_frames(
        frame_paths: &[PathBuf],
        width: usize,
        height: usize,
        fps: u32,
    ) -> Result<Vec<EncodedPacket>, VideoError> {
        if frame_paths.is_empty() {
            return Err(VideoError::NoFrames);
        }
        let mut ctx = build_context(width, height, fps)?;
        let mut packets = Vec::new();

        for path in frame_paths {
            let img = image::open(path)
                .map_err(|source| VideoError::FrameRead {
                    path: path.clone(),
                    source,
                })?
                .into_rgb8();
            if img.width() as usize != width || img.height() as usize != height {
                return Err(VideoError::FrameSizeMismatch {
                    path: path.clone(),
                    actual_width: img.width(),
                    actual_height: img.height(),
                    expected_width: width,
                    expected_height: height,
                });
            }
            let frame = frame_from_rgb(&ctx, &img);
            match ctx.send_frame(frame) {
                Ok(()) | Err(EncoderStatus::EnoughData) => {}
                Err(e) => return Err(VideoError::EncodeFailed(e)),
            }
            drain_available_packets(&mut ctx, &mut packets)?;
        }

        ctx.flush();
        loop {
            match ctx.receive_packet() {
                Ok(packet) => packets.push(EncodedPacket {
                    data: packet.data,
                    is_keyframe: packet.frame_type == FrameType::KEY,
                }),
                Err(EncoderStatus::Encoded) => continue,
                Err(EncoderStatus::LimitReached) => break,
                Err(e) => return Err(VideoError::EncodeFailed(e)),
            }
        }
        Ok(packets)
    }
}

#[cfg(feature = "av1")]
fn mux_av1(
    packets: Vec<av1_encoder::EncodedPacket>,
    width: usize,
    height: usize,
    fps: u32,
    output_path: &Path,
) -> Result<(), VideoError> {
    use muxide::api::{MuxerBuilder, VideoCodec};

    let file = std::fs::File::create(output_path)?;
    let mut muxer = MuxerBuilder::new(file)
        .video(VideoCodec::Av1, width as u32, height as u32, f64::from(fps))
        .build()
        .map_err(|e| VideoError::Mux(e.to_string()))?;

    for (i, packet) in packets.iter().enumerate() {
        let pts_secs = i as f64 / f64::from(fps);
        muxer
            .write_video(pts_secs, &packet.data, packet.is_keyframe)
            .map_err(|e| VideoError::Mux(e.to_string()))?;
    }
    muxer
        .finish_with_stats()
        .map_err(|e| VideoError::Mux(e.to_string()))?;
    Ok(())
}

/// Encodes an ordered sequence of frame files into the output video.
/// `frame_paths` must already be in playback order and every frame must be
/// exactly `picsize` (validated by the caller via `parse_picsize` before
/// this runs, and re-checked per-frame here).
#[cfg(feature = "av1")]
pub async fn encode_video(
    frame_paths: Vec<PathBuf>,
    picsize: &str,
    fps: u32,
    output_path: &Path,
) -> Result<(), VideoError> {
    let (width, height) = parse_picsize(picsize)?;
    let output_path = output_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let packets = av1_encoder::encode_frames(&frame_paths, width, height, fps)?;
        mux_av1(packets, width, height, fps, &output_path)
    })
    .await
    .map_err(|e| VideoError::Mux(e.to_string()))?
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

    #[cfg(feature = "av1")]
    #[test]
    fn encode_frames_rejects_empty_input() {
        let result = av1_encoder::encode_frames(&[], 640, 320, 20);
        assert!(matches!(result, Err(VideoError::NoFrames)));
    }

    #[cfg(feature = "av1")]
    #[test]
    fn encode_frames_rejects_mismatched_frame_size() {
        let dir = tempfile_dir();
        let path = dir.join("frame1.jpg");
        image::RgbImage::from_pixel(320, 160, image::Rgb([10, 20, 30]))
            .save(&path)
            .unwrap();
        let result = av1_encoder::encode_frames(&[path], 640, 320, 20);
        assert!(matches!(result, Err(VideoError::FrameSizeMismatch { .. })));
    }

    #[cfg(feature = "av1")]
    fn tempfile_dir() -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("svmm-video-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Fast, unignored, no network/no ffmpeg: encodes a handful of
    /// synthetic in-memory frames through the real AV1 encoder + muxide,
    /// and asserts a structurally valid MP4 comes out (correct box
    /// headers). This is what actually guards the new pipeline day to day
    /// — the `#[ignore]`d end-to-end tests in tests/pipeline.rs are
    /// skipped for billing reasons, not encoding reasons, so they are not
    /// a substitute for this.
    #[cfg(feature = "av1")]
    #[tokio::test]
    async fn encode_video_produces_a_structurally_valid_mp4() {
        let dir = tempfile_dir();
        let mut paths = Vec::new();
        for i in 0..3u8 {
            let path = dir.join(format!("frame{i}.jpg"));
            let shade = 50 + i * 40;
            image::RgbImage::from_pixel(64, 32, image::Rgb([shade, shade, shade]))
                .save(&path)
                .unwrap();
            paths.push(path);
        }
        let output_path = dir.join("out.mp4");
        encode_video(paths, "64x32", 10, &output_path)
            .await
            .expect("encode_video should succeed on valid input");

        let bytes = std::fs::read(&output_path).unwrap();
        assert!(
            bytes.len() > 8,
            "output file is too small to contain an MP4 box header"
        );
        // Every valid MP4 starts with a box whose type at offset 4..8 is
        // one of a small set of top-level box names; `ftyp` is the
        // universal first box.
        assert_eq!(
            &bytes[4..8],
            b"ftyp",
            "output is not a structurally valid MP4 (missing ftyp box)"
        );
    }
}
