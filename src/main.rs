use axum::{
    Router,
    routing::{get, post},
    middleware::from_fn,
};
use std::sync::Arc;
use tokio::signal;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod handlers;
mod error;
mod stats;
mod rate_limit;
mod health;
mod metrics;
mod retry;
mod circuit_breaker;
mod middleware;

use config::AppState;
use middleware::{request_id_middleware, logging_middleware};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "crewride=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::Config::load();
    config.validate();

    let state = Arc::new(AppState::new(config));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health::health_handler))
        .route("/ready", get(health::ready_handler))
        .route("/metrics", get(metrics::stats_handler))
        .route("/stats", get(metrics::stats_handler))
        .route("/stats/:provider", get(metrics::provider_stats_handler))
        .route("/v1/messages", post(handlers::anthropic::handler))
        .route("/v1/chat/completions", post(handlers::openai::handler))
        .route("/v1beta/models/{*path}", post(handlers::gemini::handler))
        .with_state(state.clone())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(from_fn(request_id_middleware))
        .layer(from_fn(logging_middleware));

    let addr = format!("{}:{}", state.config.host, state.config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect(&format!("Failed to bind to {}", addr));

    tracing::info!("🚀 Starting CrewRide server on http://{}", addr);
    tracing::info!("📡 Routes:");
    tracing::info!("   - GET  /health (health check)");
    tracing::info!("   - GET  /ready (readiness check)");
    tracing::info!("   - GET  /metrics (Prometheus metrics)");
    tracing::info!("   - GET  /stats (traffic statistics)");
    tracing::info!("   - POST /v1/messages (Anthropic format)");
    tracing::info!("   - POST /v1/chat/completions (OpenAI format)");
    tracing::info!("   - POST /v1beta/models/{{model}}:generateContent (Gemini format)");

    let grace_period = state.config.shutdown.grace_period_secs;
    let server = axum::serve(listener, app);

    tokio::select! {
        result = server => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = shutdown_signal() => {
            tracing::info!("Received shutdown signal, graceful shutdown in {}s...", grace_period);
            tokio::time::sleep(tokio::time::Duration::from_secs(grace_period)).await;
            tracing::info!("Shutdown complete");
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install TERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
