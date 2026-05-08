use axum::{
    extract::Request,
    http::header::HeaderName,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER.as_str())
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(request_id.clone()));

    let mut res = next.run(req).await;
    res.headers_mut().insert(REQUEST_ID_HEADER, request_id.parse().unwrap());
    res
}

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
