use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Content hash of a file, used for post-download duplicate detection.
/// Replaces the Python version's `os.system("diff ...")` shell-out.
pub fn hash_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Drops a file only when its content hash matches the immediately preceding
/// *kept* file — mirrors `prune_repeated_images_from_list`'s adjacent `diff`
/// comparison, not a global dedupe. Catches near-duplicate consecutive frames
/// that `itinerary::dedupe_by_pano_id` doesn't (different pano_id, visually
/// identical image). Returns *indices* into `paths` rather than the paths
/// themselves, so a caller can carry other per-frame data (e.g. the route
/// point a frame came from) through the same dedupe decision — a plain
/// `Vec<PathBuf>` would lose that correspondence.
pub fn dedupe_by_content_indices(paths: &[PathBuf]) -> std::io::Result<Vec<usize>> {
    let mut kept: Vec<usize> = Vec::with_capacity(paths.len());
    let mut last_hash: Option<String> = None;
    for (i, path) in paths.iter().enumerate() {
        let hash = hash_file(path)?;
        if last_hash.as_deref() != Some(hash.as_str()) {
            kept.push(i);
        }
        last_hash = Some(hash);
    }
    Ok(kept)
}

/// Copies `paths` into `dest_dir` as `{stem}0.jpg`, `{stem}1.jpg`, ... in
/// order, packing any gaps left by dedupe. Mirrors `copy_files_to_sequence`.
pub fn renumber_sequentially(
    paths: &[PathBuf],
    dest_dir: &Path,
    stem: &str,
) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dest_dir)?;
    paths
        .iter()
        .enumerate()
        .map(|(i, src)| {
            let dest = dest_dir.join(format!("{stem}{i}.jpg"));
            std::fs::copy(src, &dest)?;
            Ok(dest)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("svmm_test_lineup_{}_{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(dir: &std::path::Path, name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn hash_file_is_stable_for_identical_content() {
        let dir = temp_dir();
        let a = write_file(&dir, "a.jpg", b"same bytes");
        let b = write_file(&dir, "b.jpg", b"same bytes");
        assert_eq!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hash_file_differs_for_different_content() {
        let dir = temp_dir();
        let a = write_file(&dir, "a.jpg", b"one");
        let b = write_file(&dir, "b.jpg", b"two");
        assert_ne!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedupe_by_content_indices_drops_consecutive_duplicates() {
        let dir = temp_dir();
        let a = write_file(&dir, "0.jpg", b"same");
        let b = write_file(&dir, "1.jpg", b"same");
        let c = write_file(&dir, "2.jpg", b"different");
        let indices = dedupe_by_content_indices(&[a, b, c]).unwrap();
        assert_eq!(indices, vec![0, 2]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedupe_by_content_indices_keeps_non_consecutive_repeats() {
        let dir = temp_dir();
        let a = write_file(&dir, "0.jpg", b"same");
        let b = write_file(&dir, "1.jpg", b"different");
        let c = write_file(&dir, "2.jpg", b"same");
        let indices = dedupe_by_content_indices(&[a, b, c]).unwrap();
        assert_eq!(indices, vec![0, 1, 2]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedupe_by_content_indices_keeps_a_single_file_unchanged() {
        let dir = temp_dir();
        let a = write_file(&dir, "0.jpg", b"only");
        let indices = dedupe_by_content_indices(&[a]).unwrap();
        assert_eq!(indices, vec![0]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn renumber_sequentially_copies_files_in_order_with_new_names() {
        let src_dir = temp_dir();
        let dest_dir = temp_dir();
        let a = write_file(&src_dir, "orig_a.jpg", b"first");
        let b = write_file(&src_dir, "orig_b.jpg", b"second");
        let renumbered = renumber_sequentially(&[a, b], &dest_dir, "frame").unwrap();
        assert_eq!(
            renumbered,
            vec![dest_dir.join("frame0.jpg"), dest_dir.join("frame1.jpg")]
        );
        assert_eq!(std::fs::read(&renumbered[0]).unwrap(), b"first");
        assert_eq!(std::fs::read(&renumbered[1]).unwrap(), b"second");
        let _ = std::fs::remove_dir_all(&src_dir);
        let _ = std::fs::remove_dir_all(&dest_dir);
    }

    #[test]
    fn renumber_sequentially_leaves_the_source_files_in_place() {
        let src_dir = temp_dir();
        let dest_dir = temp_dir();
        let a = write_file(&src_dir, "orig.jpg", b"content");
        renumber_sequentially(std::slice::from_ref(&a), &dest_dir, "frame").unwrap();
        assert!(a.exists());
        let _ = std::fs::remove_dir_all(&src_dir);
        let _ = std::fs::remove_dir_all(&dest_dir);
    }
}
