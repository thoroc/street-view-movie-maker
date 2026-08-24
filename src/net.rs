use std::future::Future;
use std::time::Duration;

pub fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis(200 * 2u64.pow(attempt.min(5)))
}

/// Percent-encodes a query-parameter value (RFC 3986 unreserved set passed
/// through unescaped). Shared by modules that build request URLs by hand
/// (`streetview`, `maps`) rather than via `reqwest`'s own query encoding.
pub(crate) fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The directory raw HTTP responses are cached under, if `SVMM_HTTP_CACHE_DIR`
/// is set — a developer/debugging convenience (see `cached_fetch`), off by
/// default so it has no effect on normal runs.
pub(crate) fn http_cache_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("SVMM_HTTP_CACHE_DIR").map(std::path::PathBuf::from)
}

/// A stable, secret-free cache key for `url`: strips the `key=...` query
/// param (so cache entries survive API-key rotation and never embed a
/// secret in a filename) before hashing.
fn cache_key(url: &str) -> String {
    let redacted = match url.find("key=") {
        Some(start) => {
            let end = url[start..]
                .find('&')
                .map(|i| start + i)
                .unwrap_or(url.len());
            format!("{}key=REDACTED{}", &url[..start], &url[end..])
        }
        None => url.to_string(),
    };
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(redacted.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Reads cached bytes for `url` from `cache_dir` if present; otherwise runs
/// `fetch` and, on success, caches the result before returning it. A no-op
/// passthrough to `fetch` when `cache_dir` is `None` (the default), so this
/// has zero effect unless `SVMM_HTTP_CACHE_DIR` is explicitly set — see the
/// "Cache raw Google API responses" rule in `.claude/RULES.md` for when and
/// how to use this while debugging or iterating on a feature.
pub(crate) async fn cached_fetch<F, Fut, E>(
    url: &str,
    cache_dir: Option<&std::path::Path>,
    fetch: F,
) -> Result<Vec<u8>, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Vec<u8>, E>>,
{
    let Some(dir) = cache_dir else {
        return fetch().await;
    };
    let path = dir.join(format!("{}.bin", cache_key(url)));
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok(bytes);
    }
    let bytes = fetch().await?;
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(&path, &bytes);
    Ok(bytes)
}

/// Retries `op` up to `max_attempts` times, sleeping with exponential backoff
/// between attempts, as long as `is_transient` says the error is worth
/// retrying. Returns the last error paired with the attempt count it took.
pub async fn with_retry<F, Fut, T, E>(
    max_attempts: u32,
    mut is_transient: impl FnMut(&E) -> bool,
    mut op: F,
) -> Result<T, (u32, E)>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut attempt = 0;
    loop {
        attempt += 1;
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) if attempt < max_attempts && is_transient(&err) => {
                tokio::time::sleep(backoff_delay(attempt)).await;
            }
            Err(err) => return Err((attempt, err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[tokio::test]
    async fn succeeds_immediately_without_retrying() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_op = calls.clone();
        let result: Result<i32, (u32, &str)> = with_retry(
            3,
            |_: &&str| true,
            move || {
                let calls = calls_for_op.clone();
                async move {
                    calls.set(calls.get() + 1);
                    Ok(42)
                }
            },
        )
        .await;
        assert_eq!(result, Ok(42));
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test]
    async fn retries_transient_errors_until_success() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_op = calls.clone();
        let result: Result<i32, (u32, &str)> = with_retry(
            5,
            |_: &&str| true,
            move || {
                let calls = calls_for_op.clone();
                async move {
                    calls.set(calls.get() + 1);
                    if calls.get() < 3 {
                        Err("transient")
                    } else {
                        Ok(7)
                    }
                }
            },
        )
        .await;
        assert_eq!(result, Ok(7));
        assert_eq!(calls.get(), 3);
    }

    #[tokio::test]
    async fn gives_up_after_max_attempts() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_op = calls.clone();
        let result: Result<i32, (u32, &str)> = with_retry(
            3,
            |_: &&str| true,
            move || {
                let calls = calls_for_op.clone();
                async move {
                    calls.set(calls.get() + 1);
                    Err("always fails")
                }
            },
        )
        .await;
        assert_eq!(result, Err((3, "always fails")));
        assert_eq!(calls.get(), 3);
    }

    #[tokio::test]
    async fn does_not_retry_non_transient_errors() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_op = calls.clone();
        let result: Result<i32, (u32, &str)> = with_retry(
            5,
            |_: &&str| false,
            move || {
                let calls = calls_for_op.clone();
                async move {
                    calls.set(calls.get() + 1);
                    Err("fatal")
                }
            },
        )
        .await;
        assert_eq!(result, Err((1, "fatal")));
        assert_eq!(calls.get(), 1);
    }

    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("svmm_test_net_{}_{n}", std::process::id()))
    }

    #[test]
    fn cache_key_is_stable_across_different_key_query_values() {
        let a = cache_key("https://example.com/x?location=1,2&key=SECRET_A&fov=90");
        let b = cache_key("https://example.com/x?location=1,2&key=SECRET_B&fov=90");
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_differs_for_different_urls() {
        let a = cache_key("https://example.com/x?location=1,2&key=SECRET");
        let b = cache_key("https://example.com/x?location=3,4&key=SECRET");
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn cached_fetch_passes_through_when_no_cache_dir_is_set() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_op = calls.clone();
        let result: Result<Vec<u8>, &str> = cached_fetch("https://example.com/x", None, || {
            let calls = calls_for_op.clone();
            async move {
                calls.set(calls.get() + 1);
                Ok(vec![1, 2, 3])
            }
        })
        .await;
        assert_eq!(result, Ok(vec![1, 2, 3]));
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test]
    async fn cached_fetch_only_calls_fetch_once_across_repeated_calls() {
        let dir = temp_dir();
        let calls = Rc::new(Cell::new(0));

        for _ in 0..3 {
            let calls_for_op = calls.clone();
            let result: Result<Vec<u8>, &str> =
                cached_fetch("https://example.com/x?key=SECRET", Some(&dir), || {
                    let calls = calls_for_op.clone();
                    async move {
                        calls.set(calls.get() + 1);
                        Ok(vec![9, 9, 9])
                    }
                })
                .await;
            assert_eq!(result, Ok(vec![9, 9, 9]));
        }

        assert_eq!(calls.get(), 1, "fetch should only run on the first call");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
