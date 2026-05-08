use axum::{
    Json, http::StatusCode, response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("upstream error: {provider} - {message}")]
    UpstreamError { provider: String, message: String },

    #[error("request timeout: {provider}")]
    Timeout { provider: String },

    #[error("circuit breaker open: {provider}")]
    CircuitOpen { provider: String },

    #[error("rate limit exceeded: {limit} per {window}")]
    RateLimitExceeded { limit: u32, window: String, retry_after: u32 },

    #[error("invalid request: {field} - {message}")]
    InvalidRequest { field: String, message: String },

    #[error("configuration error: {0}")]
    ConfigError(String),

    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u32>,
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, error_type, details, retry_after) = match &self {
            ProxyError::UpstreamError { provider, message } => (
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                Some(serde_json::json!({ "provider": provider, "message": message })),
                None,
            ),
            ProxyError::Timeout { provider } => (
                StatusCode::GATEWAY_TIMEOUT,
                "timeout",
                Some(serde_json::json!({ "provider": provider })),
                None,
            ),
            ProxyError::CircuitOpen { provider } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "circuit_breaker_open",
                Some(serde_json::json!({ "provider": provider })),
                None,
            ),
            ProxyError::RateLimitExceeded { limit, window, retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_exceeded",
                Some(serde_json::json!({ "limit": limit, "window": window })),
                Some(*retry_after),
            ),
            ProxyError::InvalidRequest { field, message } => (
                StatusCode::BAD_REQUEST,
                "invalid_request",
                Some(serde_json::json!({ "field": field, "message": message })),
                None,
            ),
            ProxyError::ConfigError(msg) | ProxyError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                Some(serde_json::json!({ "message": msg })),
                None,
            ),
        };

        let mut response = Json(ErrorResponse {
            error: ErrorDetail {
                error_type: error_type.to_string(),
                message: self.to_string(),
                details,
                request_id: None,
                retry_after,
            },
        }).into_response();

        *response.status_mut() = status;
        response
    }
}

pub type ProxyResult<T> = Result<T, ProxyError>;

impl From<reqwest::Error> for ProxyError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ProxyError::Timeout {
                provider: "unknown".to_string(),
            }
        } else {
            ProxyError::UpstreamError {
                provider: "unknown".to_string(),
                message: err.to_string(),
            }
        }
    }
}

impl From<url::ParseError> for ProxyError {
    fn from(err: url::ParseError) -> Self {
        ProxyError::ConfigError(format!("URL parse error: {}", err))
    }
}
