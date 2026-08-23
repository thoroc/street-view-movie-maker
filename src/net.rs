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
}
