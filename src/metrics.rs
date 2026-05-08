use axum::{
    extract::State,
    http::HeaderValue,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use crate::config::AppState;
use serde_json::Value;

pub async fn stats_handler(State(state): State<Arc<AppState>>) -> Response {
    let stats = state.stats.get_stats().await;

    let output = serde_json::to_string_pretty(&stats).unwrap_or_default();
    let mut response = output.into_response();
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("application/json"),
    );
    response
}

pub async fn provider_stats_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(provider): axum::extract::Path<String>,
) -> Response {
    match state.stats.get_provider_stats(&provider).await {
        Some(stats) => {
            let output = serde_json::to_string_pretty(&stats).unwrap_or_default();
            let mut response = output.into_response();
            response.headers_mut().insert(
                "content-type",
                HeaderValue::from_static("application/json"),
            );
            response
        }
        None => {
            let error = Value::String("provider not found".to_string());
            let body = serde_json::json!({ "error": error }).to_string();
            let mut response = body.into_response();
            *response.status_mut() = axum::http::StatusCode::NOT_FOUND;
            response
        }
    }
}

pub fn prometheus_metrics(state: &AppState) -> String {
    let stats = futures::executor::block_on(state.stats.get_stats());

    let mut output = String::new();

    output.push_str("# HELP crewride_requests_total Total number of requests\n");
    output.push_str("# TYPE crewride_requests_total counter\n");
    output.push_str(&format!(
        "crewride_requests_total total={}\n",
        stats.overall.total_requests
    ));

    output.push_str("# HELP crewride_tokens_input_total Total input tokens\n");
    output.push_str("# TYPE crewride_tokens_input_total counter\n");
    output.push_str(&format!(
        "crewride_tokens_input_total total={}\n",
        stats.overall.total_tokens_input
    ));

    output.push_str("# HELP crewride_tokens_output_total Total output tokens\n");
    output.push_str("# TYPE crewride_tokens_output_total counter\n");
    output.push_str(&format!(
        "crewride_tokens_output_total total={}\n",
        stats.overall.total_tokens_output
    ));

    output.push_str("# HELP crewride_errors_total Total number of errors\n");
    output.push_str("# TYPE crewride_errors_total counter\n");
    output.push_str(&format!(
        "crewride_errors_total total={}\n",
        stats.overall.total_errors
    ));

    for (provider, ps) in &stats.by_provider {
        output.push_str(&format!(
            "crewride_provider_requests{{provider=\"{}\"}} {}\n",
            provider, ps.requests
        ));
        output.push_str(&format!(
            "crewride_provider_tokens_input{{provider=\"{}\"}} {}\n",
            provider, ps.tokens_input
        ));
        output.push_str(&format!(
            "crewride_provider_tokens_output{{provider=\"{}\"}} {}\n",
            provider, ps.tokens_output
        ));
        output.push_str(&format!(
            "crewride_provider_errors{{provider=\"{}\"}} {}\n",
            provider, ps.errors
        ));
    }

    for (model, ms) in &stats.by_model {
        output.push_str(&format!(
            "crewride_model_requests{{model=\"{}\"}} {}\n",
            model, ms.requests
        ));
        output.push_str(&format!(
            "crewride_model_tokens_input{{model=\"{}\"}} {}\n",
            model, ms.tokens_input
        ));
        output.push_str(&format!(
            "crewride_model_tokens_output{{model=\"{}\"}} {}\n",
            model, ms.tokens_output
        ));
        output.push_str(&format!(
            "crewride_model_errors{{model=\"{}\"}} {}\n",
            model, ms.errors
        ));
    }

    output
}
