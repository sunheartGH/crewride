use crewride::circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry, CircuitState
};
use std::sync::Arc;
use std::time::Duration;

fn create_test_breaker(failure_threshold: u32, success_threshold: u32, timeout_secs: u64) -> CircuitBreaker {
    CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold,
        success_threshold,
        timeout: Duration::from_secs(timeout_secs),
    })
}

#[test]
fn test_circuit_breaker_initial_state() {
    let breaker = create_test_breaker(5, 2, 30);
    assert!(breaker.is_allowed());
    assert_eq!(breaker.get_state(), CircuitState::Closed);
}

#[test]
fn test_circuit_breaker_success_resets() {
    let breaker = create_test_breaker(3, 2, 0);

    breaker.record_failure();
    breaker.record_failure();
    breaker.record_failure();

    assert_eq!(breaker.get_state(), CircuitState::Open);

    std::thread::sleep(Duration::from_millis(10));

    breaker.is_allowed();
    assert_eq!(breaker.get_state(), CircuitState::HalfOpen);

    breaker.record_success();
    breaker.record_success();

    assert_eq!(breaker.get_state(), CircuitState::Closed);
}

#[test]
fn test_circuit_breaker_opens_after_threshold() {
    let breaker = create_test_breaker(3, 2, 30);

    breaker.record_failure();
    assert_eq!(breaker.get_state(), CircuitState::Closed);

    breaker.record_failure();
    assert_eq!(breaker.get_state(), CircuitState::Closed);

    breaker.record_failure();
    assert_eq!(breaker.get_state(), CircuitState::Open);
    assert!(!breaker.is_allowed());
}

#[test]
fn test_circuit_breaker_half_open_after_timeout() {
    let breaker = create_test_breaker(2, 2, 0);

    breaker.record_failure();
    breaker.record_failure();
    assert_eq!(breaker.get_state(), CircuitState::Open);

    std::thread::sleep(Duration::from_millis(10));

    assert!(breaker.is_allowed());
    assert_eq!(breaker.get_state(), CircuitState::HalfOpen);
}

#[test]
fn test_circuit_breaker_half_open_to_open_on_failure() {
    let breaker = create_test_breaker(2, 2, 0);

    breaker.record_failure();
    breaker.record_failure();
    std::thread::sleep(Duration::from_millis(10));
    breaker.is_allowed();

    breaker.record_failure();

    assert_eq!(breaker.get_state(), CircuitState::Open);
}

#[test]
fn test_circuit_breaker_reset() {
    let breaker = create_test_breaker(2, 2, 30);

    breaker.record_failure();
    breaker.record_failure();
    assert_eq!(breaker.get_state(), CircuitState::Open);

    breaker.reset();

    assert_eq!(breaker.get_state(), CircuitState::Closed);
    assert!(breaker.is_allowed());
}

#[test]
fn test_circuit_breaker_config_defaults() {
    let config = CircuitBreakerConfig::default();

    assert_eq!(config.failure_threshold, 5);
    assert_eq!(config.success_threshold, 2);
    assert_eq!(config.timeout, Duration::from_secs(30));
}

#[test]
fn test_circuit_breaker_registry_new() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::default();

    let breaker1 = registry.get_or_create("provider1", config.clone());
    let breaker2 = registry.get("provider1").unwrap();

    assert!(Arc::ptr_eq(&breaker1, &breaker2));
}

#[test]
fn test_circuit_breaker_registry_different_providers() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::default();

    let breaker1 = registry.get_or_create("provider1", config.clone());
    let breaker2 = registry.get_or_create("provider2", config.clone());

    assert!(!Arc::ptr_eq(&breaker1, &breaker2));
}

#[test]
fn test_circuit_breaker_registry_get() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::default();

    registry.get_or_create("provider1", config);

    let breaker = registry.get("provider1");
    assert!(breaker.is_some());

    let unknown = registry.get("unknown");
    assert!(unknown.is_none());
}
