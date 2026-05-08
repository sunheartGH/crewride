use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use crate::config::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

pub async fn health_handler() -> Response {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }).into_response()
}

pub async fn ready_handler(State(state): State<Arc<AppState>>) -> Response {
    let providers = &state.config.providers;

    let all_enabled_providers_ready = providers
        .iter()
        .filter(|p| p.enabled)
        .all(|p| {
            state.health_status.read().get(&p.key).copied().unwrap_or(false)
        });

    if all_enabled_providers_ready || providers.is_empty() {
        Json(serde_json::json!({
            "status": "ready",
            "providers": providers.iter().map(|p| {
                serde_json::json!({
                    "key": p.key,
                    "type": p.r#type,
                    "enabled": p.enabled,
                    "healthy": state.health_status.read().get(&p.key).copied().unwrap_or(false)
                })
            }).collect::<Vec<_>>()
        })).into_response()
    } else {
        let mut response = Json(serde_json::json!({
            "status": "not_ready",
            "providers": providers.iter().map(|p| {
                serde_json::json!({
                    "key": p.key,
                    "type": p.r#type,
                    "enabled": p.enabled,
                    "healthy": state.health_status.read().get(&p.key).copied().unwrap_or(false)
                })
            }).collect::<Vec<_>>()
        })).into_response();
        *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
        response
    }
}
