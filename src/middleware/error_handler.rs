use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use crate::error::{ProxyError, ErrorResponse, ErrorDetail};

pub async fn error_handler_middleware(req: Request, next: Next) -> Response {
    let res = next.run(req).await;

    if res.status().is_server_error() {
        let error_type = match res.status() {
            StatusCode::BAD_GATEWAY => "upstream_error",
            StatusCode::GATEWAY_TIMEOUT => "timeout",
            StatusCode::SERVICE_UNAVAILABLE => "circuit_breaker_open",
            StatusCode::TOO_MANY_REQUESTS => "rate_limit_exceeded",
            _ => "internal_error",
        };

        let error_response = ErrorResponse {
            error: ErrorDetail {
                error_type: error_type.to_string(),
                message: format!("HTTP {}", res.status().as_u16()),
                details: None,
                request_id: None,
                retry_after: None,
            },
        };

        return (res.status(), Json(error_response)).into_response();
    }

    res
}
