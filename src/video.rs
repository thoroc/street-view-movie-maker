use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error(
        "ffmpeg not found on PATH — install it and ensure it's callable from the command line: {0}"
    )]
    FfmpegMissing(String),
    #[error("failed to launch ffmpeg: {0}")]
    Spawn(std::io::Error),
    #[error("ffmpeg exited with an error:\n{stderr}")]
    Failed { stderr: String },
}

/// Builds the ffmpeg argument list, reproducing the original Python
/// `make_video` command for parity (`libx264`, `-crf 25`, `yuv420p`).
pub fn build_ffmpeg_args(
    lineup_dir: &Path,
    stem: &str,
    fps: u32,
    picsize: &str,
    output_path: &Path,
) -> Vec<String> {
    vec![
        "-r".to_string(),
        fps.to_string(),
        "-f".to_string(),
        "image2".to_string(),
        "-s".to_string(),
        picsize.to_string(),
        "-i".to_string(),
        format!("{}/{stem}%d.jpg", lineup_dir.display()),
        "-vcodec".to_string(),
        "libx264".to_string(),
        "-crf".to_string(),
        "25".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        output_path.display().to_string(),
        "-y".to_string(),
    ]
}

/// Startup check (see the plan's Startup checks) — verifies ffmpeg is on
/// PATH before any billed API calls happen.
pub async fn check_ffmpeg_available() -> Result<(), VideoError> {
    tokio::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .await
        .map(|_| ())
        .map_err(|e| VideoError::FfmpegMissing(e.to_string()))
}

/// Encodes the lineup frames into a video via ffmpeg. Only `build_ffmpeg_args`
/// is unit tested directly; actually invoking ffmpeg against real frames is
/// exercised by the phase 6 end-to-end test, not here.
pub async fn encode_video(
    lineup_dir: &Path,
    stem: &str,
    fps: u32,
    picsize: &str,
    output_path: &Path,
) -> Result<(), VideoError> {
    let args = build_ffmpeg_args(lineup_dir, stem, fps, picsize, output_path);
    let output = tokio::process::Command::new("ffmpeg")
        .args(&args)
        .output()
        .await
        .map_err(VideoError::Spawn)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(VideoError::Failed {
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn build_ffmpeg_args_matches_python_flag_parity() {
        let args = build_ffmpeg_args(
            Path::new("/tmp/lineup"),
            "frame",
            24,
            "640x640",
            Path::new("/tmp/out.mp4"),
        );
        assert_eq!(
            args,
            vec![
                "-r",
                "24",
                "-f",
                "image2",
                "-s",
                "640x640",
                "-i",
                "/tmp/lineup/frame%d.jpg",
                "-vcodec",
                "libx264",
                "-crf",
                "25",
                "-pix_fmt",
                "yuv420p",
                "/tmp/out.mp4",
                "-y",
            ]
        );
    }

    #[tokio::test]
    async fn check_ffmpeg_available_succeeds_when_ffmpeg_is_on_path() {
        // This project's own mise.toml pins ffmpeg as a required tool, so
        // it's reasonable to assert this passes in any dev/CI environment
        // that follows the plan's tooling setup — not just an aspirational
        // check.
        assert!(check_ffmpeg_available().await.is_ok());
    }
}
