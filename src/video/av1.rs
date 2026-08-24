use super::{VideoError, parse_picsize, rgb_to_yuv420_bt709};
use rav1e::prelude::*;
use std::path::{Path, PathBuf};

/// One encoded AV1 temporal unit (OBU stream) plus whether it's a
/// keyframe — muxide's `write_video` needs the latter explicitly since
/// it can't be inferred from the OBU bytes without re-parsing them.
struct EncodedPacket {
    data: Vec<u8>,
    is_keyframe: bool,
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
fn encode_frames(
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
            std::env::temp_dir().join(format!("svmm-video-av1-test-{}-{n}", std::process::id()));
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

    /// Fast, unignored, no network/no ffmpeg: encodes a handful of
    /// synthetic in-memory frames through the real AV1 encoder + muxide,
    /// and asserts a structurally valid MP4 comes out (correct box
    /// headers). This is what actually guards the new pipeline day to day
    /// — the `#[ignore]`d end-to-end tests in tests/pipeline.rs are
    /// skipped for billing reasons, not encoding reasons, so they are not
    /// a substitute for this.
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
