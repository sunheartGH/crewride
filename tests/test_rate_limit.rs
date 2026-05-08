use crewride::rate_limit::{RateLimiter, RateLimitConfig, RateLimitResult};

fn create_test_limiter(rpm: u32, tpm: u32) -> RateLimiter {
    RateLimiter::new(RateLimitConfig {
        enabled: true,
        default_rpm: rpm,
        default_tpm: tpm,
        burst: 0,
    })
}

#[test]
fn test_rate_limiter_disabled() {
    let limiter = RateLimiter::new(RateLimitConfig {
        enabled: false,
        default_rpm: 1,
        default_tpm: 100,
        burst: 0,
    });

    for _ in 0..100 {
        assert!(matches!(
            limiter.check_rate_limit("test_key", 100),
            RateLimitResult::Allowed
        ));
    }
}

#[test]
fn test_rate_limiter_basic_request_limit() {
    let limiter = create_test_limiter(5, 1000);

    for i in 0..5 {
        let result = limiter.check_rate_limit("test_key", 1);
        assert!(matches!(result, RateLimitResult::Allowed),
            "Request {} should be allowed", i + 1);
    }

    let result = limiter.check_rate_limit("test_key", 1);
    assert!(matches!(result, RateLimitResult::Denied { .. }),
        "Request 6 should be denied");
}

#[test]
fn test_rate_limiter_token_limit() {
    let limiter = create_test_limiter(100, 5);

    for i in 0..5 {
        let result = limiter.check_rate_limit("test_key", 1);
        assert!(matches!(result, RateLimitResult::Allowed),
            "Request with token {} should be allowed", i + 1);
    }

    let result = limiter.check_rate_limit("test_key", 1);
    assert!(matches!(result, RateLimitResult::Denied { .. }),
        "Request with token 6 should be denied");
}

#[test]
fn test_rate_limiter_different_keys_independent() {
    let limiter = create_test_limiter(2, 1000);

    assert!(matches!(
        limiter.check_rate_limit("key1", 1),
        RateLimitResult::Allowed
    ));
    assert!(matches!(
        limiter.check_rate_limit("key1", 1),
        RateLimitResult::Allowed
    ));
    assert!(matches!(
        limiter.check_rate_limit("key1", 1),
        RateLimitResult::Denied { .. }
    ));

    assert!(matches!(
        limiter.check_rate_limit("key2", 1),
        RateLimitResult::Allowed
    ));
}

#[test]
fn test_rate_limit_result_denied_contains_retry_info() {
    let limiter = create_test_limiter(1, 1000);

    limiter.check_rate_limit("test_key", 1);

    let result = limiter.check_rate_limit("test_key", 1);
    match result {
        RateLimitResult::Denied { limit: _, retry_after } => {
            assert!(retry_after > 0);
        }
        _ => panic!("Expected Denied result"),
    }
}

#[test]
fn test_rate_limit_config_defaults() {
    let config = RateLimitConfig::default();

    assert!(config.enabled);
    assert_eq!(config.default_rpm, 60);
    assert_eq!(config.default_tpm, 100000);
    assert_eq!(config.burst, 10);
}

#[test]
fn test_rate_limiter_refill_on_wait() {
    use std::time::Duration;
    use std::thread;

    let limiter = create_test_limiter(60, 1000);

    assert!(matches!(
        limiter.check_rate_limit("refill_key", 1),
        RateLimitResult::Allowed
    ));

    thread::sleep(Duration::from_millis(100));

    assert!(matches!(
        limiter.check_rate_limit("refill_key", 1),
        RateLimitResult::Allowed
    ));
}
