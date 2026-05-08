use std::time::Duration;
use tokio::time::sleep;
use tracing::{warn, info};

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

pub async fn with_retry<F, Fut, T>(config: &RetryConfig, mut operation: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut attempts = 0;
    let mut delay = config.initial_delay;

    loop {
        attempts += 1;

        match operation().await {
            Ok(result) => {
                if attempts > 1 {
                    info!("operation succeeded after {} attempts", attempts);
                }
                return Ok(result);
            }
            Err(e) if attempts >= config.max_attempts => {
                warn!(
                    "operation failed after {} attempts: {}",
                    attempts, e
                );
                return Err(e);
            }
            Err(e) => {
                warn!(
                    "operation failed (attempt {}), retrying in {:?}: {}",
                    attempts, delay, e
                );
                sleep(delay).await;
                delay = std::cmp::min(
                    Duration::from_secs_f64(delay.as_secs_f64() * config.backoff_multiplier),
                    config.max_delay,
                );
            }
        }
    }
}
