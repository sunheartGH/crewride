use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderStats {
    pub requests: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelStats {
    pub requests: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OverallStats {
    pub total_requests: u64,
    pub total_tokens_input: u64,
    pub total_tokens_output: u64,
    pub total_errors: u64,
}

#[derive(Debug)]
struct AtomicGroup {
    requests: AtomicU64,
    tokens_input: AtomicU64,
    tokens_output: AtomicU64,
    errors: AtomicU64,
}

impl Default for AtomicGroup {
    fn default() -> Self {
        Self {
            requests: AtomicU64::new(0),
            tokens_input: AtomicU64::new(0),
            tokens_output: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StatsCollector {
    inner: Arc<StatsInner>,
}

#[derive(Debug, Default)]
struct StatsInner {
    total: AtomicGroup,
    by_provider: RwLock<HashMap<String, Arc<AtomicGroup>>>,
    by_model: RwLock<HashMap<String, Arc<AtomicGroup>>>,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(StatsInner::default()),
        }
    }

    pub fn record_request(&self, provider: &str, model: &str) {
        self.inner.total.requests.fetch_add(1, Ordering::Relaxed);
        self.add_to_map(&self.inner.by_provider, provider).requests.fetch_add(1, Ordering::Relaxed);
        self.add_to_map(&self.inner.by_model, model).requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_tokens(&self, provider: &str, model: &str, input: u64, output: u64) {
        self.inner.total.tokens_input.fetch_add(input, Ordering::Relaxed);
        self.inner.total.tokens_output.fetch_add(output, Ordering::Relaxed);

        self.add_to_map(&self.inner.by_provider, provider).tokens_input.fetch_add(input, Ordering::Relaxed);
        self.add_to_map(&self.inner.by_provider, provider).tokens_output.fetch_add(output, Ordering::Relaxed);

        self.add_to_map(&self.inner.by_model, model).tokens_input.fetch_add(input, Ordering::Relaxed);
        self.add_to_map(&self.inner.by_model, model).tokens_output.fetch_add(output, Ordering::Relaxed);
    }

    pub fn record_error(&self, provider: &str, model: &str) {
        self.inner.total.errors.fetch_add(1, Ordering::Relaxed);
        self.add_to_map(&self.inner.by_provider, provider).errors.fetch_add(1, Ordering::Relaxed);
        self.add_to_map(&self.inner.by_model, model).errors.fetch_add(1, Ordering::Relaxed);
    }

    fn add_to_map(&self, map: &RwLock<HashMap<String, Arc<AtomicGroup>>>, key: &str) -> Arc<AtomicGroup> {
        if let Ok(read_guard) = map.try_read() {
            if let Some(group) = read_guard.get(key) {
                return group.clone();
            }
        }

        if let Ok(mut write_guard) = map.try_write() {
            let group = Arc::new(AtomicGroup::default());
            write_guard.insert(key.to_string(), group.clone());
            return group;
        }

        Arc::new(AtomicGroup::default())
    }

    pub async fn get_stats(&self) -> StatsSnapshot {
        let by_provider = self.inner.by_provider.read().await;
        let by_model = self.inner.by_model.read().await;

        StatsSnapshot {
            overall: OverallStats {
                total_requests: self.inner.total.requests.load(Ordering::Relaxed),
                total_tokens_input: self.inner.total.tokens_input.load(Ordering::Relaxed),
                total_tokens_output: self.inner.total.tokens_output.load(Ordering::Relaxed),
                total_errors: self.inner.total.errors.load(Ordering::Relaxed),
            },
            by_provider: by_provider
                .iter()
                .map(|(k, v)| (k.clone(), ProviderStats {
                    requests: v.requests.load(Ordering::Relaxed),
                    tokens_input: v.tokens_input.load(Ordering::Relaxed),
                    tokens_output: v.tokens_output.load(Ordering::Relaxed),
                    errors: v.errors.load(Ordering::Relaxed),
                }))
                .collect(),
            by_model: by_model
                .iter()
                .map(|(k, v)| (k.clone(), ModelStats {
                    requests: v.requests.load(Ordering::Relaxed),
                    tokens_input: v.tokens_input.load(Ordering::Relaxed),
                    tokens_output: v.tokens_output.load(Ordering::Relaxed),
                    errors: v.errors.load(Ordering::Relaxed),
                }))
                .collect(),
        }
    }

    pub async fn get_provider_stats(&self, provider: &str) -> Option<ProviderStats> {
        let map = self.inner.by_provider.read().await;
        map.get(provider).map(|v| ProviderStats {
            requests: v.requests.load(Ordering::Relaxed),
            tokens_input: v.tokens_input.load(Ordering::Relaxed),
            tokens_output: v.tokens_output.load(Ordering::Relaxed),
            errors: v.errors.load(Ordering::Relaxed),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    #[serde(flatten)]
    pub overall: OverallStats,
    pub by_provider: HashMap<String, ProviderStats>,
    pub by_model: HashMap<String, ModelStats>,
}
