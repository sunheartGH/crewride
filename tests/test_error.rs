use crewride::error::{ProxyError, ErrorResponse, ErrorDetail};
use axum::http::StatusCode;
use axum::response::IntoResponse;

#[test]
fn test_proxy_error_display() {
    let error = ProxyError::UpstreamError {
        provider: "openai".to_string(),
        message: "connection timeout".to_string(),
    };
    assert_eq!(error.to_string(), "upstream error: openai - connection timeout");

    let error = ProxyError::Timeout {
        provider: "anthropic".to_string(),
    };
    assert_eq!(error.to_string(), "request timeout: anthropic");

    let error = ProxyError::CircuitOpen {
        provider: "gemini".to_string(),
    };
    assert_eq!(error.to_string(), "circuit breaker open: gemini");

    let error = ProxyError::RateLimitExceeded {
        limit: 60,
        window: "per minute".to_string(),
        retry_after: 30,
    };
    assert_eq!(error.to_string(), "rate limit exceeded: 60 per per minute");

    let error = ProxyError::InvalidRequest {
        field: "model".to_string(),
        message: "missing required field".to_string(),
    };
    assert_eq!(error.to_string(), "invalid request: model - missing required field");

    let error = ProxyError::ConfigError("invalid config".to_string());
    assert_eq!(error.to_string(), "configuration error: invalid config");

    let error = ProxyError::Internal("something went wrong".to_string());
    assert_eq!(error.to_string(), "internal error: something went wrong");
}

#[test]
fn test_proxy_error_into_response() {
    let error = ProxyError::UpstreamError {
        provider: "openai".to_string(),
        message: "test error".to_string(),
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let error = ProxyError::Timeout {
        provider: "openai".to_string(),
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);

    let error = ProxyError::CircuitOpen {
        provider: "openai".to_string(),
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let error = ProxyError::RateLimitExceeded {
        limit: 60,
        window: "per minute".to_string(),
        retry_after: 30,
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let error = ProxyError::InvalidRequest {
        field: "model".to_string(),
        message: "test".to_string(),
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let error = ProxyError::ConfigError("test".to_string());
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let error = ProxyError::Internal("test".to_string());
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn test_error_response_serialization() {
    let response = ErrorResponse {
        error: ErrorDetail {
            error_type: "upstream_error".to_string(),
            message: "test message".to_string(),
            details: Some(serde_json::json!({"provider": "openai"})),
            request_id: Some("req-123".to_string()),
            retry_after: Some(30),
        },
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"upstream_error\""));
    assert!(json.contains("\"message\":\"test message\""));
    assert!(json.contains("\"request_id\":\"req-123\""));
    assert!(json.contains("\"retry_after\":30"));
}

#[test]
fn test_error_response_optional_fields() {
    let response = ErrorResponse {
        error: ErrorDetail {
            error_type: "internal_error".to_string(),
            message: "test".to_string(),
            details: None,
            request_id: None,
            retry_after: None,
        },
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(!json.contains("\"details\":null"));
    assert!(!json.contains("\"request_id\":null"));
    assert!(!json.contains("\"retry_after\":null"));
}

#[test]
fn test_from_url_parse_error() {
    let url_err = url::ParseError::EmptyHost;
    let proxy_error: ProxyError = url_err.into();
    assert!(matches!(proxy_error, ProxyError::ConfigError(_)));
}
