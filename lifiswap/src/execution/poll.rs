//! Generic async polling with configurable retry logic.

use std::future::Future;
use std::time::Duration;

/// Poll an async function until it returns `Some(T)` or retries are exhausted.
///
/// # Arguments
///
/// * `f` — Async function to poll. Returns `Ok(Some(T))` when done, `Ok(None)` to retry.
/// * `interval` — Delay between attempts.
/// * `max_retries` — Maximum number of retry attempts (0 = try once).
///
/// # Errors
///
/// Returns:
/// - The error from `f` if it fails.
/// - `None` (via the outer `Option`) if retries are exhausted without a result.
///
/// # Example
///
/// ```ignore
/// let result = wait_for_result(
///     || async { Ok(Some(42)) },
///     Duration::from_secs(1),
///     10,
/// ).await;
/// ```
pub async fn wait_for_result<T, E, F, Fut>(
    f: F,
    interval: Duration,
    max_retries: u32,
) -> Result<Option<T>, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<Option<T>, E>>,
{
    for _ in 0..=max_retries {
        match f().await {
            Ok(Some(value)) => return Ok(Some(value)),
            Ok(None) => {
                tokio::time::sleep(interval).await;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    #[tokio::test]
    async fn returns_immediately_on_first_success() {
        let result = wait_for_result(
            || async { Ok::<_, &str>(Some(42)) },
            Duration::from_millis(10),
            5,
        )
        .await;
        assert_eq!(result, Ok(Some(42)));
    }

    #[tokio::test]
    async fn retries_until_success() {
        let counter = AtomicU32::new(0);
        let result = wait_for_result(
            || async {
                let n = counter.fetch_add(1, Ordering::Relaxed);
                if n < 3 {
                    Ok::<_, &str>(None)
                } else {
                    Ok(Some("done"))
                }
            },
            Duration::from_millis(1),
            5,
        )
        .await;
        assert_eq!(result, Ok(Some("done")));
        assert_eq!(counter.load(Ordering::Relaxed), 4);
    }

    #[tokio::test]
    async fn returns_none_when_exhausted() {
        let result = wait_for_result(
            || async { Ok::<Option<i32>, &str>(None) },
            Duration::from_millis(1),
            2,
        )
        .await;
        assert_eq!(result, Ok(None));
    }

    #[tokio::test]
    async fn propagates_error_immediately() {
        let counter = AtomicU32::new(0);
        let result = wait_for_result(
            || async {
                counter.fetch_add(1, Ordering::Relaxed);
                Err::<Option<i32>, _>("boom")
            },
            Duration::from_millis(1),
            5,
        )
        .await;
        assert_eq!(result, Err("boom"));
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }
}
