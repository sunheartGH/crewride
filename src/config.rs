use std::path::Path;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use url::Url;
use aidapter::Provider;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub models: Vec<ModelConfig>,
    #[serde(default)]
    pub stats: StatsConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub retry: RetryConfig,
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
    #[serde(default)]
    pub timeout: TimeoutConfig,
    #[serde(default)]
    pub shutdown: ShutdownConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_retention_days() -> u32 { 30 }
fn default_true() -> bool { true }

impl Default for StatsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: default_retention_days(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_true")]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_initial_delay")]
    pub initial_delay_ms: u64,
    #[serde(default = "default_max_delay")]
    pub max_delay_ms: u64,
    #[serde(default = "default_backoff")]
    pub backoff_multiplier: f64,
}

fn default_max_attempts() -> u32 { 3 }
fn default_initial_delay() -> u64 { 100 }
fn default_max_delay() -> u64 { 5000 }
fn default_backoff() -> f64 { 2.0 }

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            initial_delay_ms: default_initial_delay(),
            max_delay_ms: default_max_delay(),
            backoff_multiplier: default_backoff(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_success_threshold")]
    pub success_threshold: u32,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_failure_threshold() -> u32 { 5 }
fn default_success_threshold() -> u32 { 2 }
fn default_timeout() -> u64 { 30 }

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_failure_threshold(),
            success_threshold: default_success_threshold(),
            timeout_secs: default_timeout(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeoutConfig {
    #[serde(default = "default_connect_timeout")]
    pub connect_secs: u64,
    #[serde(default = "default_read_timeout")]
    pub read_secs: u64,
    #[serde(default = "default_write_timeout")]
    pub write_secs: u64,
}

fn default_connect_timeout() -> u64 { 10 }
fn default_read_timeout() -> u64 { 60 }
fn default_write_timeout() -> u64 { 60 }

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect_secs: default_connect_timeout(),
            read_secs: default_read_timeout(),
            write_secs: default_write_timeout(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ShutdownConfig {
    #[serde(default = "default_grace_period")]
    pub grace_period_secs: u64,
}

fn default_grace_period() -> u64 { 30 }

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            grace_period_secs: default_grace_period(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub key: String,
    pub name: String,
    pub r#type: Provider,
    pub api_key: Option<String>,
    pub api_url: Option<Url>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub rate_limit: Option<ProviderRateLimitConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRateLimitConfig {
    pub rpm: Option<u32>,
    pub tpm: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model: String,
    pub name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub replace: Option<ReplaceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceConfig {
    #[serde(default)]
    pub api_key: bool,
    pub model: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8899
}

impl Config {
    pub fn load() -> Config {
        if let Ok(config_path) = std::env::var("CREWRIDE_CONFIG_FILE") {
            println!("📄 Loading config from: {}", config_path);
            match Config::from_file(&config_path) {
                Ok(config) => {
                    println!("✅ Config loaded successfully");
                    return config;
                }
                Err(e) => {
                    eprintln!("❌ Failed to load config: {}", e);
                    eprintln!("   Falling back to environment variables");
                }
            }
        }

        if let Ok(current_dir) = std::env::current_dir() {
            println!("🔍 Searching for config files in current directory");
            match Config::from_dir(&current_dir) {
                Ok(config) => {
                    println!("✅ Config loaded automatically");
                    return config;
                }
                Err(e) => {
                    println!("📝 No config file found: {}", e);
                }
            }
        }

        let mut config = Config::default();
        if let Err(e) = config.merge_env() {
            eprintln!("⚠️  Warning: Failed to merge environment variables: {}", e);
        }
        config
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let mut config: Config = match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            .as_deref()
        {
            Some("json") => {
                println!("📄 Loading JSON config from: {}", path.display());
                serde_json::from_str(&contents).with_context(|| {
                    format!("Failed to parse JSON config from: {}", path.display())
                })?
            }
            Some("yaml") | Some("yml") => {
                println!("📄 Loading YAML config from: {}", path.display());
                serde_yaml::from_str(&contents).with_context(|| {
                    format!("Failed to parse YAML config from: {}", path.display())
                })?
            }
            Some(ext) => return Err(anyhow!("Unsupported file extension: .{}", ext)),
            None => return Err(anyhow!("File must have an extension (.json or .yaml)")),
        };

        config.merge_env().with_context(|| {
            format!("Failed to merge environment variables for config: {}", path.display())
        })?;

        Ok(config)
    }

    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        println!("🔍 Searching for config files in: {}", dir.display());

        let candidates = ["config.json", "config.yaml", "config.yml"];

        for filename in &candidates {
            let config_path = dir.join(filename);
            if config_path.exists() {
                println!("✅ Found config file: {}", config_path.display());
                return Self::from_file(config_path);
            }
        }

        Err(anyhow!("No config file found in directory: {}", dir.display()))
    }

    pub fn merge_env(&mut self) -> Result<()> {
        for provider in &mut self.providers {
            match provider.r#type {
                Provider::OpenAI => {
                    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
                        provider.api_key = Some(api_key);
                    }
                    if let Ok(api_url) = std::env::var("OPENAI_API_URL") {
                        provider.api_url = Some(api_url.parse().with_context(|| "Invalid OPENAI_API_URL format")?);
                    } else if provider.api_url.is_none() {
                        provider.api_url = Some(openai_api_url()?);
                    }
                }
                Provider::Anthropic => {
                    if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
                        provider.api_key = Some(api_key);
                    }
                    if let Ok(api_url) = std::env::var("ANTHROPIC_API_URL") {
                        provider.api_url = Some(api_url.parse().with_context(|| "Invalid ANTHROPIC_API_URL format")?);
                    } else if provider.api_url.is_none() {
                        provider.api_url = Some(anthropic_api_url()?);
                    }
                }
                Provider::Gemini => {
                    if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
                        provider.api_key = Some(api_key);
                    }
                    if let Ok(api_url) = std::env::var("GEMINI_API_URL") {
                        provider.api_url = Some(api_url.parse().with_context(|| "Invalid GEMINI_API_URL format")?);
                    } else if provider.api_url.is_none() {
                        provider.api_url = Some(gemini_api_url()?);
                    }
                }
            }
        }

        if let Ok(host) = std::env::var("CREWRIDE_PROXY_HOST") {
            self.host = host;
        }

        if let Ok(port) = std::env::var("CREWRIDE_PROXY_PORT") {
            if let Ok(port_num) = port.parse() {
                self.port = port_num;
            }
        }

        Ok(())
    }

    pub fn validate(&self) {
        for provider in &self.providers {
            if provider.enabled && provider.api_key.is_none() {
                eprintln!("⚠️  Warning: {} Api Key not set", provider.key);
            }
        }
    }

    pub fn find_model(&self, model: &str) -> Option<&ModelConfig> {
        self.models.iter().find(|m| m.model == model)
    }

    pub fn find_provider(&self, key: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.key == key && p.enabled)
    }

    pub fn give_provider(&self, r#type: Provider) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.r#type == r#type && p.enabled)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: default_host(),
            port: default_port(),
            providers: Vec::new(),
            models: Vec::new(),
            stats: StatsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            retry: RetryConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            timeout: TimeoutConfig::default(),
            shutdown: ShutdownConfig::default(),
        }
    }
}

fn openai_api_url() -> Result<Url> {
    "https://api.openai.com/"
        .parse::<Url>()
        .context("Failed to parse OpenAI API URL")
}

fn anthropic_api_url() -> Result<Url> {
    "https://api.anthropic.com/"
        .parse::<Url>()
        .context("Failed to parse Anthropic API URL")
}

fn gemini_api_url() -> Result<Url> {
    "https://generativelanguage.googleapis.com/"
        .parse::<Url>()
        .context("Failed to parse Gemini API URL")
}

#[derive(Clone)]
pub struct AppState {
    pub client: reqwest::Client,
    pub config: Config,
    pub stats: crate::stats::StatsCollector,
    pub rate_limiter: crate::rate_limit::RateLimiter,
    pub circuit_breakers: crate::circuit_breaker::CircuitBreakerRegistry,
    pub health_status: Arc<RwLock<std::collections::HashMap<String, bool>>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let timeout = config.timeout;
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(timeout.connect_secs))
            .read_timeout(std::time::Duration::from_secs(timeout.read_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
            stats: crate::stats::StatsCollector::new(),
            rate_limiter: crate::rate_limit::RateLimiter::new(
                crate::rate_limit::RateLimitConfig {
                    enabled: true,
                    default_rpm: 60,
                    default_tpm: 100000,
                    burst: 10,
                }
            ),
            circuit_breakers: crate::circuit_breaker::CircuitBreakerRegistry::new(),
            health_status: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

pub struct ProxyState {
    pub client: reqwest::Client,
    pub config: Config,
}
