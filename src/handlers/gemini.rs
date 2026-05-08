use std::sync::Arc;
use std::collections::HashMap;
use axum::{
    Json, response::Response, http::StatusCode,
    extract::{State, Path, Query}, response::IntoResponse,
};
use url::Url;
use aidapter::{
    Provider,
    gemini::prefix::GeminiChatRequest,
    openai::prefix::OpenAIChatRequest,
    anthropic::prefix::AnthropicChatRequest,
};
use crate::config::AppState;
use crate::error::ProxyError;

pub mod openai;
pub mod anthropic;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(req): Json<GeminiChatRequest>,
) -> Result<Response, ProxyError> {
    let (mut model, method) = match path.rsplit_once(':') {
        Some((m, method)) => (m.to_string(), method),
        None => return Err(ProxyError::InvalidRequest {
            field: "path".to_string(),
            message: "Invalid path format, expected {model}:{method}".to_string(),
        }),
    };

    let mut api_key = query.get("key").cloned().unwrap_or_default();
    let mut provider_config = None;
    let mut replace_config = None;

    if let Some(model_config) = state.config.find_model(&model) {
        if let Some(provider) = &model_config.provider {
            replace_config = model_config.replace.clone();
            provider_config = state.config.find_provider(provider);
        }
    }

    if provider_config.is_none() {
        provider_config = state.config.give_provider(Provider::Gemini);
    }

    let provider_config = provider_config.ok_or_else(|| ProxyError::ConfigError(
        "No Gemini provider configured".to_string()
    ))?;

    let mut replace_api_key = api_key.is_empty();

    if let Some(ref replace_cfg) = replace_config {
        replace_api_key = replace_cfg.api_key;
        if let Some(model_name) = &replace_cfg.model {
            model = model_name.clone();
        }
    }

    if replace_api_key {
        api_key = provider_config.api_key.clone()
            .ok_or_else(|| ProxyError::ConfigError("API key not configured".to_string()))?;
    }

    let api_url = provider_config.api_url.clone()
        .ok_or_else(|| ProxyError::ConfigError("API URL not configured".to_string()))?;

    let provider_key = &provider_config.key;

    state.stats.record_request(provider_key, &model);

    if state.config.rate_limit.enabled {
        let limit_result = state.rate_limiter.check_rate_limit(&api_key, 1);
        if let crate::rate_limit::RateLimitResult::Denied { retry_after, .. } = limit_result {
            return Err(ProxyError::RateLimitExceeded {
                limit: 60,
                window: "per minute".to_string(),
                retry_after,
            });
        }
    }

    let circuit_breaker = state.circuit_breakers.get_or_create(
        provider_key,
        crate::circuit_breaker::CircuitBreakerConfig {
            failure_threshold: state.config.circuit_breaker.failure_threshold,
            success_threshold: state.config.circuit_breaker.success_threshold,
            timeout: std::time::Duration::from_secs(state.config.circuit_breaker.timeout_secs),
        }
    );

    if !circuit_breaker.is_allowed() {
        return Err(ProxyError::CircuitOpen { provider: provider_key.clone() });
    }

    let result = match method {
        "generateContent" => {
            match provider_config.r#type {
                Provider::OpenAI => into_openai(state, model, req, api_url, api_key).await,
                Provider::Anthropic => into_anthropic(state, model, req, api_url, api_key).await,
                Provider::Gemini => straight(state, model, req, api_url, api_key).await,
            }
        },
        "streamGenerateContent" => {
            match provider_config.r#type {
                Provider::OpenAI => stream::into_openai(state, model, req, api_url, api_key).await,
                Provider::Anthropic => stream::into_anthropic(state, model, req, api_url, api_key).await,
                Provider::Gemini => stream::straight(state, model, req, api_url, api_key).await,
            }
        },
        _ => Err(ProxyError::InvalidRequest {
            field: "method".to_string(),
            message: format!("Unknown method: {}", method),
        }),
    };

    match &result {
        Ok(_) => circuit_breaker.record_success(),
        Err(_) => circuit_breaker.record_failure(),
    }

    result
}

async fn straight(
    state: Arc<AppState>,
    model: String,
    req: GeminiChatRequest,
    api_url: Url,
    api_key: String,
) -> Result<Response, ProxyError> {
    let url = api_url
        .join(&format!("/v1beta/models/{}:generateContent?key={}", model, api_key))
        .map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?;

    let response = state
        .client
        .post(url.as_str())
        .header("content-type", "application/json")
        .json(&req)
        .send()
        .await
        .map_err(|e| ProxyError::UpstreamError {
            provider: "gemini".to_string(),
            message: e.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ProxyError::UpstreamError {
            provider: "gemini".to_string(),
            message: format!("HTTP {}: {}", status.as_u16(), body),
        });
    }

    let resp: serde_json::Value = response.json().await.map_err(|_| ProxyError::Internal("Failed to parse response".to_string()))?;
    Ok(Json(resp).into_response())
}

async fn into_openai(
    state: Arc<AppState>,
    model: String,
    req: GeminiChatRequest,
    api_url: Url,
    api_key: String,
) -> Result<Response, ProxyError> {
    let mut openai_req = OpenAIChatRequest::from(&req);
    openai_req.model = model;

    let response = state
        .client
        .post(
            api_url.join("/v1/chat/completions").map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?.as_str(),
        )
        .header("authorization", format!("Bearer {}", api_key))
        .header("content-type", "application/json")
        .json(&openai_req)
        .send()
        .await
        .map_err(|e| ProxyError::UpstreamError {
            provider: "openai".to_string(),
            message: e.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ProxyError::UpstreamError {
            provider: "openai".to_string(),
            message: format!("HTTP {}: {}", status.as_u16(), body),
        });
    }

    openai::from_openai_response(response).await
}

async fn into_anthropic(
    state: Arc<AppState>,
    model: String,
    req: GeminiChatRequest,
    api_url: Url,
    api_key: String,
) -> Result<Response, ProxyError> {
    let mut anthropic_req = AnthropicChatRequest::from(&req);
    anthropic_req.model = model;

    let response = state
        .client
        .post(
            api_url.join("/v1/messages").map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?.as_str(),
        )
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&anthropic_req)
        .send()
        .await
        .map_err(|e| ProxyError::UpstreamError {
            provider: "anthropic".to_string(),
            message: e.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ProxyError::UpstreamError {
            provider: "anthropic".to_string(),
            message: format!("HTTP {}: {}", status.as_u16(), body),
        });
    }

    anthropic::from_anthropic_response(response).await
}

pub mod stream {
    use std::sync::Arc;
    use axum::{
        response::Response, http::StatusCode,
        response::IntoResponse, body::Body,
    };
    use url::Url;
    use aidapter::{
        gemini::prefix::GeminiChatRequest,
        openai::prefix::OpenAIChatRequest,
        anthropic::prefix::AnthropicChatRequest,
    };
    use crate::config::AppState;
    use crate::error::ProxyError;

    use super::{openai, anthropic};

    pub async fn straight(
        state: Arc<AppState>,
        model: String,
        req: GeminiChatRequest,
        api_url: Url,
        api_key: String,
    ) -> Result<Response, ProxyError> {
        let url = api_url
            .join(&format!("/v1beta/models/{}:streamGenerateContent?key={}&alt=sse", model, api_key))
            .map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?;

        let response = state
            .client
            .post(url.as_str())
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| ProxyError::UpstreamError {
                provider: "gemini".to_string(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProxyError::UpstreamError {
                provider: "gemini".to_string(),
                message: format!("HTTP {}: {}", status.as_u16(), body),
            });
        }

        let stream = response.bytes_stream();
        let body = Body::from_stream(stream);
        Ok((StatusCode::OK, [("content-type", "text/event-stream")], body).into_response())
    }

    pub async fn into_openai(
        state: Arc<AppState>,
        model: String,
        req: GeminiChatRequest,
        api_url: Url,
        api_key: String,
    ) -> Result<Response, ProxyError> {
        let mut openai_req = OpenAIChatRequest::from(&req);
        openai_req.model = model;
        openai_req.stream = Some(true);

        let response = state
            .client
            .post(
                api_url.join("/v1/chat/completions").map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?.as_str(),
            )
            .header("authorization", format!("Bearer {}", api_key))
            .header("content-type", "application/json")
            .json(&openai_req)
            .send()
            .await
            .map_err(|e| ProxyError::UpstreamError {
                provider: "openai".to_string(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProxyError::UpstreamError {
                provider: "openai".to_string(),
                message: format!("HTTP {}: {}", status.as_u16(), body),
            });
        }

        openai::from_openai_streaming(response).await
    }

    pub async fn into_anthropic(
        state: Arc<AppState>,
        model: String,
        req: GeminiChatRequest,
        api_url: Url,
        api_key: String,
    ) -> Result<Response, ProxyError> {
        let mut anthropic_req = AnthropicChatRequest::from(&req);
        anthropic_req.model = model;
        anthropic_req.stream = Some(true);

        let response = state
            .client
            .post(
                api_url.join("/v1/messages").map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?.as_str(),
            )
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_req)
            .send()
            .await
            .map_err(|e| ProxyError::UpstreamError {
                provider: "anthropic".to_string(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProxyError::UpstreamError {
                provider: "anthropic".to_string(),
                message: format!("HTTP {}: {}", status.as_u16(), body),
            });
        }

        anthropic::from_anthropic_streaming(response).await
    }
}
