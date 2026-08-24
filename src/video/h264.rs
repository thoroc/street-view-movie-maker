use super::{VideoError, parse_picsize, rgb_to_yuv420_bt709};
use openh264::OpenH264API;
use openh264::encoder::{Encoder, EncoderConfig, FrameRate, FrameType};
use openh264::formats::YUVSource;
use std::path::{Path, PathBuf};

/// One encoded H.264 access unit (Annex B NAL units, start-code
/// delimited) plus whether it's a keyframe — mirrors the AV1 backend's
/// own packet type; muxide needs both codecs' packets in the same shape.
struct EncodedPacket {
    data: Vec<u8>,
    is_keyframe: bool,
}

/// Adapts our own BT.709 YUV planes to openh264's `YUVSource` trait,
/// bypassing openh264's own RGB->YUV conversion so both codec paths use
/// the same colour science (see `rgb_to_yuv420_bt709`'s doc comment).
struct PlanarYuv420 {
    width: usize,
    height: usize,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl YUVSource for PlanarYuv420 {
    fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn strides(&self) -> (usize, usize, usize) {
        (self.width, self.width / 2, self.width / 2)
    }

    fn y(&self) -> &[u8] {
        &self.y
    }

    fn u(&self) -> &[u8] {
        &self.u
    }

    fn v(&self) -> &[u8] {
        &self.v
    }
}

fn build_encoder(fps: u32) -> Result<Encoder, VideoError> {
    // `EncoderConfig::new()`, not `::default()` — the latter is a
    // `#[derive(Default)]` that zeroes every field (0bps bitrate, an
    // empty QP range), which OpenH264 rejects at init with
    // `cmInitParaError`. `new()` is the crate's own hand-tuned set of
    // sane defaults.
    //
    // `skip_frames(false)`: `new()`'s default lets the rate controller
    // drop a frame it judges near-identical to the previous one, which
    // it does often on real, closely-spaced Street View frames — the
    // skipped frame's encode() call returns a zero-length payload, and
    // muxide rejects a zero-length video sample ("video frame N is
    // empty"). Every input frame must produce a real sample here, so
    // skip is disabled outright rather than filtered around.
    let config = EncoderConfig::new()
        .max_frame_rate(FrameRate::from_hz(fps as f32))
        .skip_frames(false);
    Encoder::with_api_config(OpenH264API::from_source(), config)
        .map_err(|e| VideoError::EncoderInit(e.to_string()))
}

/// Encodes an ordered sequence of same-sized frame files into H.264
/// access units. `frame_paths` must already be in playback order — see
/// the AV1 backend's `encode_frames` doc comment; the same guarantee
/// from `lineup::renumber_sequentially` applies here.
fn encode_frames(
    frame_paths: &[PathBuf],
    width: usize,
    height: usize,
    fps: u32,
) -> Result<Vec<EncodedPacket>, VideoError> {
    if frame_paths.is_empty() {
        return Err(VideoError::NoFrames);
    }
    let mut encoder = build_encoder(fps)?;
    let mut packets = Vec::with_capacity(frame_paths.len());

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
        let (y, u, v) = rgb_to_yuv420_bt709(&img);
        let source = PlanarYuv420 {
            width,
            height,
            y,
            u,
            v,
        };
        let bitstream = encoder.encode(&source).map_err(VideoError::EncodeFailed)?;
        packets.push(EncodedPacket {
            data: bitstream.to_vec(),
            is_keyframe: bitstream.frame_type() == FrameType::IDR,
        });
    }
    Ok(packets)
}

fn mux(
    packets: Vec<EncodedPacket>,
    width: usize,
    height: usize,
    fps: u32,
    output_path: &Path,
) -> Result<(), VideoError> {
    use muxide::api::{MuxerBuilder, VideoCodec};

    let file = std::fs::File::create(output_path)?;
    let mut muxer = MuxerBuilder::new(file)
        .video(
            VideoCodec::H264,
            width as u32,
            height as u32,
            f64::from(fps),
        )
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

/// Encodes an ordered sequence of frame files into the output video via the
/// opt-in H.264 backend — see the AV1 backend's `encode_video` for the
/// shared contract on `frame_paths` ordering and size validation.
pub async fn encode_video(
    frame_paths: Vec<PathBuf>,
    picsize: &str,
    fps: u32,
    output_path: &Path,
) -> Result<(), VideoError> {
    let (width, height) = parse_picsize(picsize)?;
    let output_path = output_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let packets = encode_frames(&frame_paths, width, height, fps)?;
        mux(packets, width, height, fps, &output_path)
    })
    .await
    .map_err(|e| VideoError::Mux(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempfile_dir() -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("svmm-video-h264-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn encode_frames_rejects_empty_input() {
        let result = encode_frames(&[], 640, 320, 20);
        assert!(matches!(result, Err(VideoError::NoFrames)));
    }

    #[test]
    fn encode_frames_rejects_mismatched_frame_size() {
        let dir = tempfile_dir();
        let path = dir.join("frame1.jpg");
        image::RgbImage::from_pixel(320, 160, image::Rgb([10, 20, 30]))
            .save(&path)
            .unwrap();
        let result = encode_frames(&[path], 640, 320, 20);
        assert!(matches!(result, Err(VideoError::FrameSizeMismatch { .. })));
    }

    /// Mirrors the AV1 backend's own structurally-valid-MP4 test — see
    /// Phase 2 Step 4 of the replace-ffmpeg plan, which calls for a
    /// real-encode test over muxide's H.264 path rather than assuming it
    /// behaves like the AV1 path.
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
        assert_eq!(
            &bytes[4..8],
            b"ftyp",
            "output is not a structurally valid MP4 (missing ftyp box)"
        );
    }
}
