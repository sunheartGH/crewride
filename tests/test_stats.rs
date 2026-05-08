use crewride::stats::StatsCollector;

#[tokio::test]
async fn test_stats_collector_new() {
    let collector = StatsCollector::new();
    let stats = collector.get_stats().await;

    assert_eq!(stats.overall.total_requests, 0);
    assert_eq!(stats.overall.total_tokens_input, 0);
    assert_eq!(stats.overall.total_tokens_output, 0);
    assert_eq!(stats.overall.total_errors, 0);
    assert!(stats.by_provider.is_empty());
    assert!(stats.by_model.is_empty());
}

#[tokio::test]
async fn test_record_request() {
    let collector = StatsCollector::new();

    collector.record_request("openai", "gpt-4");
    collector.record_request("openai", "gpt-4");
    collector.record_request("anthropic", "claude-3");

    let stats = collector.get_stats().await;

    assert_eq!(stats.overall.total_requests, 3);
    assert_eq!(stats.by_provider.get("openai").unwrap().requests, 2);
    assert_eq!(stats.by_provider.get("anthropic").unwrap().requests, 1);
    assert_eq!(stats.by_model.get("gpt-4").unwrap().requests, 2);
    assert_eq!(stats.by_model.get("claude-3").unwrap().requests, 1);
}

#[tokio::test]
async fn test_record_tokens() {
    let collector = StatsCollector::new();

    collector.record_tokens("openai", "gpt-4", 100, 50);
    collector.record_tokens("openai", "gpt-4", 200, 100);

    let stats = collector.get_stats().await;

    assert_eq!(stats.overall.total_tokens_input, 300);
    assert_eq!(stats.overall.total_tokens_output, 150);
    assert_eq!(stats.by_provider.get("openai").unwrap().tokens_input, 300);
    assert_eq!(stats.by_provider.get("openai").unwrap().tokens_output, 150);
    assert_eq!(stats.by_model.get("gpt-4").unwrap().tokens_input, 300);
    assert_eq!(stats.by_model.get("gpt-4").unwrap().tokens_output, 150);
}

#[tokio::test]
async fn test_record_error() {
    let collector = StatsCollector::new();

    collector.record_error("openai", "gpt-4");
    collector.record_error("anthropic", "claude-3");

    let stats = collector.get_stats().await;

    assert_eq!(stats.overall.total_errors, 2);
    assert_eq!(stats.by_provider.get("openai").unwrap().errors, 1);
    assert_eq!(stats.by_provider.get("anthropic").unwrap().errors, 1);
}

#[tokio::test]
async fn test_get_provider_stats() {
    let collector = StatsCollector::new();

    collector.record_request("openai", "gpt-4");
    collector.record_request("openai", "gpt-3.5");
    collector.record_tokens("openai", "gpt-4", 100, 50);

    let openai_stats = collector.get_provider_stats("openai").await;
    assert!(openai_stats.is_some());
    let stats = openai_stats.unwrap();
    assert_eq!(stats.requests, 2);
    assert_eq!(stats.tokens_input, 100);

    let unknown_stats = collector.get_provider_stats("unknown").await;
    assert!(unknown_stats.is_none());
}

#[tokio::test]
async fn test_concurrent_requests() {
    use std::sync::Arc;
    use tokio::task;

    let collector = Arc::new(StatsCollector::new());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let collector = collector.clone();
            task::spawn(async move {
                for _ in 0..100 {
                    collector.record_request("provider", "model");
                }
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let stats = collector.get_stats().await;
    assert_eq!(stats.overall.total_requests, 1000);
}
