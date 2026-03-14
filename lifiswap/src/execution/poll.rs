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
