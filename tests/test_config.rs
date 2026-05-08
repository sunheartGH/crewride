use crewride::config::{
    Config, ProviderConfig, ModelConfig, ReplaceConfig,
    StatsConfig, RateLimitConfig, RetryConfig, CircuitBreakerConfig,
    TimeoutConfig, ShutdownConfig, ProviderRateLimitConfig
};
use aidapter::Provider;
use url::Url;

#[test]
fn test_config_defaults() {
    let config = Config::default();

    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8899);
    assert!(config.providers.is_empty());
    assert!(config.models.is_empty());
}

#[test]
fn test_stats_config_defaults() {
    let config = StatsConfig::default();

    assert!(config.enabled);
    assert_eq!(config.retention_days, 30);
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
fn test_retry_config_defaults() {
    let config = RetryConfig::default();

    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.initial_delay_ms, 100);
    assert_eq!(config.max_delay_ms, 5000);
    assert_eq!(config.backoff_multiplier, 2.0);
}

#[test]
fn test_circuit_breaker_config_defaults() {
    let config = CircuitBreakerConfig::default();

    assert_eq!(config.failure_threshold, 5);
    assert_eq!(config.success_threshold, 2);
    assert_eq!(config.timeout_secs, 30);
}

#[test]
fn test_timeout_config_defaults() {
    let config = TimeoutConfig::default();

    assert_eq!(config.connect_secs, 10);
    assert_eq!(config.read_secs, 60);
    assert_eq!(config.write_secs, 60);
}

#[test]
fn test_shutdown_config_defaults() {
    let config = ShutdownConfig::default();

    assert_eq!(config.grace_period_secs, 30);
}

#[test]
fn test_provider_config() {
    let provider = ProviderConfig {
        key: "openai-official".to_string(),
        name: "OpenAI Official".to_string(),
        r#type: Provider::OpenAI,
        api_key: Some("sk-test".to_string()),
        api_url: Url::parse("https://api.openai.com/").ok(),
        enabled: true,
        rate_limit: Some(ProviderRateLimitConfig {
            rpm: Some(100),
            tpm: Some(200000),
        }),
    };

    assert_eq!(provider.key, "openai-official");
    assert_eq!(provider.name, "OpenAI Official");
    assert_eq!(provider.r#type, Provider::OpenAI);
    assert!(provider.api_key.is_some());
    assert!(provider.enabled);
    assert!(provider.rate_limit.is_some());
}

#[test]
fn test_model_config() {
    let model = ModelConfig {
        model: "gpt-4".to_string(),
        name: Some("GPT-4".to_string()),
        provider: Some("openai-official".to_string()),
        replace: Some(ReplaceConfig {
            api_key: true,
            model: Some("gpt-4o".to_string()),
        }),
    };

    assert_eq!(model.model, "gpt-4");
    assert!(model.name.is_some());
    assert_eq!(model.provider, Some("openai-official".to_string()));
    assert!(model.replace.is_some());
}

#[test]
fn test_replace_config_defaults() {
    let replace = ReplaceConfig {
        api_key: false,
        model: None,
    };

    assert!(!replace.api_key);
    assert!(replace.model.is_none());
}

#[test]
fn test_provider_type_serialization() {
    let json_openai = serde_json::to_string(&Provider::OpenAI).unwrap();
    assert_eq!(json_openai, "\"openai\"");

    let json_anthropic = serde_json::to_string(&Provider::Anthropic).unwrap();
    assert_eq!(json_anthropic, "\"anthropic\"");

    let json_gemini = serde_json::to_string(&Provider::Gemini).unwrap();
    assert_eq!(json_gemini, "\"gemini\"");
}

#[test]
fn test_provider_type_deserialization() {
    let openai: Provider = serde_json::from_str("\"openai\"").unwrap();
    assert_eq!(openai, Provider::OpenAI);

    let anthropic: Provider = serde_json::from_str("\"anthropic\"").unwrap();
    assert_eq!(anthropic, Provider::Anthropic);

    let gemini: Provider = serde_json::from_str("\"gemini\"").unwrap();
    assert_eq!(gemini, Provider::Gemini);
}

#[test]
fn test_config_json_serialization() {
    let config = Config {
        host: "0.0.0.0".to_string(),
        port: 8080,
        providers: vec![ProviderConfig {
            key: "test".to_string(),
            name: "Test".to_string(),
            r#type: Provider::OpenAI,
            api_key: None,
            api_url: None,
            enabled: true,
            rate_limit: None,
        }],
        models: vec![],
        stats: StatsConfig::default(),
        rate_limit: RateLimitConfig::default(),
        retry: RetryConfig::default(),
        circuit_breaker: CircuitBreakerConfig::default(),
        timeout: TimeoutConfig::default(),
        shutdown: ShutdownConfig::default(),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    assert!(json.contains("0.0.0.0"));
    assert!(json.contains("8080"));
    assert!(json.contains("openai"));
}

#[test]
fn test_find_provider() {
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 8899,
        providers: vec![
            ProviderConfig {
                key: "openai-official".to_string(),
                name: "OpenAI".to_string(),
                r#type: Provider::OpenAI,
                api_key: None,
                api_url: None,
                enabled: true,
                rate_limit: None,
            },
            ProviderConfig {
                key: "anthropic-official".to_string(),
                name: "Anthropic".to_string(),
                r#type: Provider::Anthropic,
                api_key: None,
                api_url: None,
                enabled: false,
                rate_limit: None,
            },
        ],
        models: vec![],
        stats: StatsConfig::default(),
        rate_limit: RateLimitConfig::default(),
        retry: RetryConfig::default(),
        circuit_breaker: CircuitBreakerConfig::default(),
        timeout: TimeoutConfig::default(),
        shutdown: ShutdownConfig::default(),
    };

    let provider = config.find_provider("openai-official");
    assert!(provider.is_some());
    assert_eq!(provider.unwrap().key, "openai-official");

    let disabled = config.find_provider("anthropic-official");
    assert!(disabled.is_none());
}

#[test]
fn test_give_provider_by_type() {
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 8899,
        providers: vec![
            ProviderConfig {
                key: "openai-official".to_string(),
                name: "OpenAI".to_string(),
                r#type: Provider::OpenAI,
                api_key: None,
                api_url: None,
                enabled: true,
                rate_limit: None,
            },
        ],
        models: vec![],
        stats: StatsConfig::default(),
        rate_limit: RateLimitConfig::default(),
        retry: RetryConfig::default(),
        circuit_breaker: CircuitBreakerConfig::default(),
        timeout: TimeoutConfig::default(),
        shutdown: ShutdownConfig::default(),
    };

    let provider = config.give_provider(Provider::OpenAI);
    assert!(provider.is_some());

    let unknown = config.give_provider(Provider::Anthropic);
    assert!(unknown.is_none());
}

#[test]
fn test_find_model() {
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 8899,
        providers: vec![],
        models: vec![
            ModelConfig {
                model: "gpt-4".to_string(),
                name: None,
                provider: None,
                replace: None,
            },
            ModelConfig {
                model: "claude-3".to_string(),
                name: None,
                provider: None,
                replace: None,
            },
        ],
        stats: StatsConfig::default(),
        rate_limit: RateLimitConfig::default(),
        retry: RetryConfig::default(),
        circuit_breaker: CircuitBreakerConfig::default(),
        timeout: TimeoutConfig::default(),
        shutdown: ShutdownConfig::default(),
    };

    let model = config.find_model("gpt-4");
    assert!(model.is_some());
    assert_eq!(model.unwrap().model, "gpt-4");

    let unknown = config.find_model("unknown");
    assert!(unknown.is_none());
}
