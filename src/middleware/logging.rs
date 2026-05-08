use axum::{
    extract::Request,
    http::Uri,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{info, warn, error, Level};
use crate::middleware::request_id::RequestId;

pub async fn logging_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri: Uri = req.uri().clone();
    let request_id = req.extensions().get::<RequestId>().map(|id| id.0.clone()).unwrap_or_default();

    info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        "request started"
    );

    let response = next.run(req).await;
    let elapsed = start.elapsed();
    let status = response.status();

    if status.is_success() {
        info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status,
            elapsed_ms = elapsed.as_millis(),
            "request completed"
        );
    } else if status.is_server_error() {
        error!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status,
            elapsed_ms = elapsed.as_millis(),
            "request failed with server error"
        );
    } else {
        warn!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status,
            elapsed_ms = elapsed.as_millis(),
            "request completed with client error"
        );
    }

    response
}
