use std::sync::Arc;
use axum::{
    Json, response::Response, http::StatusCode,
    extract::State, body::Body, response::IntoResponse,
    http::{ HeaderMap, header::AUTHORIZATION }
};
use url::Url;
use aidapter::{
    Provider,
    openai::prefix::OpenAIChatRequest,
    anthropic::prefix::AnthropicChatRequest,
    gemini::prefix::GeminiChatRequest,
};
use crate::config::AppState;
use crate::error::ProxyError;

pub mod anthropic;
pub mod gemini;

pub async fn handler(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<OpenAIChatRequest>,
) -> Result<Response, ProxyError> {
    let request_key = headers.get(AUTHORIZATION)
        .and_then(|auth| auth.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .map(|key| key.to_string())
        .unwrap_or_default();

    let mut provider_config = None;
    let mut replace_config = None;

    if let Some(model_config) = state.config.find_model(&req.model) {
        if let Some(provider) = &model_config.provider {
            replace_config = model_config.replace.clone();
            provider_config = state.config.find_provider(provider);
        }
    }

    if provider_config.is_none() {
        provider_config = state.config.give_provider(Provider::OpenAI);
    }

    let provider_config = provider_config.ok_or_else(|| ProxyError::ConfigError(
        "No OpenAI provider configured".to_string()
    ))?;

    let mut replace_api_key = request_key.is_empty();

    if let Some(ref replace_cfg) = replace_config {
        replace_api_key = replace_cfg.api_key;
        if let Some(model_name) = &replace_cfg.model {
            req.model = model_name.clone();
        }
    }

    let api_key = if replace_api_key {
        provider_config.api_key.clone()
            .ok_or_else(|| ProxyError::ConfigError("API key not configured".to_string()))?
    } else {
        request_key
    };

    let api_url = provider_config.api_url.clone()
        .ok_or_else(|| ProxyError::ConfigError("API URL not configured".to_string()))?;

    let provider_key = &provider_config.key;
    let is_streaming = req.stream.unwrap_or(false);

    state.stats.record_request(provider_key, &req.model);

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

    let result = match provider_config.r#type {
        Provider::OpenAI => straight(state, req, api_url, api_key).await,
        Provider::Anthropic => into_anthropic(state, req, api_url, api_key).await,
        Provider::Gemini => into_gemini(state, req, api_url, api_key).await,
    };

    match &result {
        Ok(_) => circuit_breaker.record_success(),
        Err(_) => circuit_breaker.record_failure(),
    }

    result
}

async fn straight(
    state: Arc<AppState>,
    req: OpenAIChatRequest,
    api_url: Url,
    api_key: String,
) -> Result<Response, ProxyError> {
    let is_streaming = req.stream.unwrap_or(false);

    let response = state
        .client
        .post(
            api_url.join("/v1/chat/completions").map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?.as_str(),
        )
        .header("authorization", format!("Bearer {}", api_key))
        .header("content-type", "application/json")
        .json(&req)
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

    if is_streaming {
        let stream = response.bytes_stream();
        let body = Body::from_stream(stream);
        Ok((StatusCode::OK, [("content-type", "text/event-stream")], body).into_response())
    } else {
        let resp: serde_json::Value = response.json().await.map_err(|_| ProxyError::Internal("Failed to parse response".to_string()))?;
        Ok(Json(resp).into_response())
    }
}

async fn into_anthropic(
    state: Arc<AppState>,
    req: OpenAIChatRequest,
    api_url: Url,
    api_key: String,
) -> Result<Response, ProxyError> {
    let is_streaming = req.stream.unwrap_or(false);

    let response = state
        .client
        .post(
            api_url.join("/v1/messages").map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?.as_str(),
        )
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&AnthropicChatRequest::from(&req))
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

    if is_streaming {
        anthropic::from_anthropic_streaming(response).await
    } else {
        anthropic::from_anthropic_response(response).await
    }
}

async fn into_gemini(
    state: Arc<AppState>,
    req: OpenAIChatRequest,
    api_url: Url,
    api_key: String,
) -> Result<Response, ProxyError> {
    let is_streaming = req.stream.unwrap_or(false);
    let gemini_req = GeminiChatRequest::from(&req);

    let endpoint = if is_streaming {
        format!("/v1beta/models/{}:streamGenerateContent?key={}&alt=sse", req.model, api_key)
    } else {
        format!("/v1beta/models/{}:generateContent?key={}", req.model, api_key)
    };

    let response = state
        .client
        .post(
            api_url.join(&endpoint).map_err(|_| ProxyError::ConfigError("Invalid URL".to_string()))?.as_str(),
        )
        .header("content-type", "application/json")
        .json(&gemini_req)
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

    if is_streaming {
        gemini::from_gemini_streaming(response).await
    } else {
        gemini::from_gemini_response(response).await
    }
}
