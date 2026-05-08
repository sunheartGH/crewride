use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
pub struct CircuitBreaker {
    state: RwLock<CircuitState>,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: RwLock<Option<Instant>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: RwLock::new(None),
            config,
        }
    }

    pub fn is_allowed(&self) -> bool {
        let state = self.state.read();

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                drop(state);
                let last_failure = self.last_failure_time.read();
                if let Some(time) = *last_failure {
                    if time.elapsed() >= self.config.timeout {
                        *self.state.write() = CircuitState::HalfOpen;
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_success(&self) {
        let mut state = self.state.write();

        match *state {
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                let successes = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if successes >= self.config.success_threshold as u64 {
                    *state = CircuitState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                }
            }
            CircuitState::Open => {}
        }
    }

    pub fn record_failure(&self) {
        let mut state = self.state.write();

        *self.last_failure_time.write() = Some(Instant::now());

        match *state {
            CircuitState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if failures >= self.config.failure_threshold as u64 {
                    *state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                *state = CircuitState::Open;
                self.success_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {}
        }
    }

    pub fn get_state(&self) -> CircuitState {
        *self.state.read()
    }

    pub fn reset(&self) {
        let mut state = self.state.write();
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        *self.last_failure_time.write() = None;
    }
}

#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerRegistry {
    breakers: Arc<RwLock<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
}

impl CircuitBreakerRegistry {
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn get_or_create(&self, name: &str, config: CircuitBreakerConfig) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.write();
        if let Some(cb) = breakers.get(name) {
            return cb.clone();
        }
        let cb = Arc::new(CircuitBreaker::new(config));
        breakers.insert(name.to_string(), cb.clone());
        cb
    }

    pub fn get(&self, name: &str) -> Option<Arc<CircuitBreaker>> {
        self.breakers.read().get(name).cloned()
    }
}
