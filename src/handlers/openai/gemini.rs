use axum::{
    Json, response::Response, http::StatusCode, 
    body::Body, response::IntoResponse
};
use futures::StreamExt;

use aidapter::{
    gemini::prefix::{GeminiChatResponse, GeminiStreamChunk},
    openai::prefix::{OpenAIChatResponse, OpenAIStreamChunk},
};
use crate::error::ProxyError;

pub async fn from_gemini_streaming(
    response: reqwest::Response,
) -> Result<Response, ProxyError> {
    use eventsource_stream::Eventsource;

    let byte_stream = response.bytes_stream();
    let event_stream = byte_stream.eventsource();

    let openai_stream = event_stream.filter_map(|result| async move {
        match result {
            Ok(event) => {
                if event.data == "[DONE]" {
                    return Some(Ok::<_, std::io::Error>(b"data: [DONE]\n\n".to_vec()));
                }

                let gemini_chunk: GeminiStreamChunk = match serde_json::from_str(&event.data) {
                    Ok(chunk) => chunk,
                    Err(_) => return None,
                };

                let openai_chunks = Vec::<OpenAIStreamChunk>::from(&gemini_chunk);
                
                let bytes: Vec<u8> = openai_chunks
                    .iter()
                    .flat_map(|chunk| Vec::<u8>::from(chunk))
                    .collect();
                
                Some(Ok::<_, std::io::Error>(bytes))
            }
            Err(_) => None,
        }
    });

    let body = Body::from_stream(openai_stream);
    Ok((StatusCode::OK, [("content-type", "text/event-stream")], body).into_response())
}

pub async fn from_gemini_response(response: reqwest::Response) -> Result<Response, ProxyError> {
    let resp: GeminiChatResponse = response
        .json()
        .await
        .map_err(|_| ProxyError::Internal("Failed to parse response".to_string()))?;

    Ok(Json(OpenAIChatResponse::from(&resp)).into_response())
}
