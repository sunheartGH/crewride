use std::sync::atomic::{AtomicU64, Ordering};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    #[serde(default = "default_rpm")]
    pub default_rpm: u32,
    #[serde(default = "default_tpm")]
    pub default_tpm: u32,
    #[serde(default = "default_burst")]
    pub burst: u32,
}

fn default_rpm() -> u32 { 60 }
fn default_tpm() -> u32 { 100000 }
fn default_burst() -> u32 { 10 }

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_rpm: default_rpm(),
            default_tpm: default_tpm(),
            burst: default_burst(),
        }
    }
}

#[derive(Debug, Clone)]
struct Bucket {
    tokens: f64,
    last_update: std::time::Instant,
    max_tokens: f64,
    refill_rate: f64,
}

impl Bucket {
    fn new(max: u32, refill_per_second: f64) -> Self {
        Self {
            tokens: max as f64,
            last_update: std::time::Instant::now(),
            max_tokens: max as f64,
            refill_rate: refill_per_second,
        }
    }

    fn try_acquire(&mut self, amount: u32) -> bool {
        self.refill();
        if self.tokens >= amount as f64 {
            self.tokens -= amount as f64;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_update = now;
    }

    fn available(&self) -> u32 {
        self.tokens.ceil() as u32
    }

    fn time_to_refill(&self, amount: u32) -> std::time::Duration {
        if self.tokens >= amount as f64 {
            std::time::Duration::ZERO
        } else {
            let needed = (amount as f64 - self.tokens) / self.refill_rate;
            std::time::Duration::from_secs_f64(needed.ceil())
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    request_buckets: Arc<RwLock<HashMap<String, Bucket>>>,
    token_buckets: Arc<RwLock<HashMap<String, Bucket>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            request_buckets: Arc::new(RwLock::new(HashMap::new())),
            token_buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub fn check_rate_limit(&self, key: &str, tokens_needed: u32) -> RateLimitResult {
        if !self.config.enabled {
            return RateLimitResult::Allowed;
        }

        let mut requests = self.request_buckets.write();
        let mut tokens = self.token_buckets.write();

        let request_bucket = requests.entry(key.to_string()).or_insert_with(|| {
            Bucket::new(self.config.default_rpm, self.config.default_rpm as f64 / 60.0)
        });

        let token_bucket = tokens.entry(key.to_string()).or_insert_with(|| {
            Bucket::new(self.config.default_tpm, self.config.default_tpm as f64 / 60.0)
        });

        if !request_bucket.try_acquire(1) {
            let retry_after = request_bucket.time_to_refill(1).as_secs() as u32;
            return RateLimitResult::Denied {
                limit: request_bucket.available(),
                retry_after,
            };
        }

        if !token_bucket.try_acquire(tokens_needed) {
            let retry_after = token_bucket.time_to_refill(tokens_needed).as_secs() as u32;
            return RateLimitResult::Denied {
                limit: token_bucket.available(),
                retry_after,
            };
        }

        RateLimitResult::Allowed
    }

    pub fn get_status(&self, key: &str) -> RateLimitStatus {
        let requests = self.request_buckets.read();
        let tokens = self.token_buckets.read();

        RateLimitStatus {
            requests_available: requests.get(key).map(|b| b.available()).unwrap_or(self.config.default_rpm),
            tokens_available: tokens.get(key).map(|b| b.available()).unwrap_or(self.config.default_tpm),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RateLimitResult {
    Allowed,
    Denied { limit: u32, retry_after: u32 },
}

#[derive(Debug, Clone, Serialize)]
pub struct RateLimitStatus {
    pub requests_available: u32,
    pub tokens_available: u32,
}
