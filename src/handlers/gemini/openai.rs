use axum::{
    Json, response::Response, http::StatusCode, 
    body::Body, response::IntoResponse
};
use futures::StreamExt;

use aidapter::{
    openai::prefix::{OpenAIChatResponse, OpenAIStreamChunk},
    gemini::prefix::{GeminiChatResponse, GeminiStreamChunk},
};
use crate::error::ProxyError;

pub async fn from_openai_streaming(
    response: reqwest::Response,
) -> Result<Response, ProxyError> {
    use eventsource_stream::Eventsource;

    let byte_stream = response.bytes_stream();
    let event_stream = byte_stream.eventsource();

    let gemini_stream = event_stream.filter_map(|result| async move {
        match result {
            Ok(event) => {
                if event.data == "[DONE]" {
                    return None;
                }

                let openai_chunk: OpenAIStreamChunk = match serde_json::from_str(&event.data) {
                    Ok(chunk) => chunk,
                    Err(_) => return None,
                };

                let gemini_chunks = Vec::<GeminiStreamChunk>::from(&openai_chunk);
                
                let bytes: Vec<u8> = gemini_chunks
                    .iter()
                    .flat_map(|chunk| {
                        let json = serde_json::to_string(chunk).unwrap_or_default();
                        format!("data: {}\n\n", json).into_bytes()
                    })
                    .collect();
                
                Some(Ok::<_, std::io::Error>(bytes))
            }
            Err(_) => None,
        }
    });

    let body = Body::from_stream(gemini_stream);
    Ok((StatusCode::OK, [("content-type", "text/event-stream")], body).into_response())
}

pub async fn from_openai_response(response: reqwest::Response) -> Result<Response, ProxyError> {
    let resp: OpenAIChatResponse = response
        .json()
        .await
        .map_err(|_| ProxyError::Internal("Failed to parse response".to_string()))?;

    Ok(Json(GeminiChatResponse::from(&resp)).into_response())
}
