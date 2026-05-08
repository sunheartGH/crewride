use crewride::retry::{RetryConfig, with_retry};
use std::time::Duration;

#[tokio::test]
async fn test_retry_config_defaults() {
    let config = RetryConfig::default();

    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.initial_delay, Duration::from_millis(100));
    assert_eq!(config.max_delay, Duration::from_secs(5));
    assert_eq!(config.backoff_multiplier, 2.0);
}

#[tokio::test]
async fn test_retry_success_first_try() {
    let config = RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(100),
        backoff_multiplier: 2.0,
    };

    let result = with_retry(&config, || async {
        Ok::<_, String>("success".to_string())
    }).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
}

#[tokio::test]
async fn test_retry_failure_after_max_attempts() {
    let config = RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(50),
        backoff_multiplier: 2.0,
    };

    let result = with_retry(&config, || async {
        Err::<String, _>("always fail".to_string())
    }).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "always fail");
}

#[tokio::test]
async fn test_retry_success_on_retry() {
    let config = RetryConfig {
        max_attempts: 5,
        initial_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(50),
        backoff_multiplier: 2.0,
    };

    let mut attempts = 0;

    let result = with_retry(&config, || {
        attempts += 1;
        async move {
            if attempts < 3 {
                Err::<String, _>("not ready".to_string())
            } else {
                Ok("ready".to_string())
            }
        }
    }).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "ready");
    assert_eq!(attempts, 3);
}

#[tokio::test]
async fn test_retry_exponential_backoff() {
    let config = RetryConfig {
        max_attempts: 5,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_millis(1000),
        backoff_multiplier: 2.0,
    };

    let start = std::time::Instant::now();
    let mut attempts = 0;

    let _ = with_retry(&config, || {
        attempts += 1;
        async move {
            if attempts < 4 {
                Err::<(), _>("retry".to_string())
            } else {
                Ok(())
            }
        }
    }).await;

    let elapsed = start.elapsed();

    assert!(elapsed >= Duration::from_millis(700),
        "Expected at least 700ms backoff (100 + 200 + 400), got {:?}", elapsed);
}
