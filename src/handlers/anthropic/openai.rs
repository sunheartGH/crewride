use axum::{
    Json, response::Response, http::StatusCode, 
    body::Body, response::IntoResponse
};
use futures::StreamExt;

use aidapter::{
    openai::prefix::{OpenAIChatResponse, OpenAIStreamChunk},
    anthropic::prefix::{AnthropicChatResponse, AnthropicStreamChunk},
};
use crate::error::ProxyError;

pub async fn from_openai_streaming(
    response: reqwest::Response,
) -> Result<Response, ProxyError> {
    use eventsource_stream::Eventsource;

    let byte_stream = response.bytes_stream();
    let event_stream = byte_stream.eventsource();

    let anthropic_stream = event_stream.filter_map(|result| async move {
        match result {
            Ok(event) => {
                if event.data == "[DONE]" {
                    return None;
                }

                let openai_chunk: OpenAIStreamChunk = match serde_json::from_str(&event.data) {
                    Ok(chunk) => chunk,
                    Err(_) => return None,
                };

                let events = Vec::<AnthropicStreamChunk>::from(&openai_chunk);
                
                Some(Ok::<_, std::io::Error>(
                    events.into_iter().flat_map(|s| Vec::<u8>::from(&s)).collect::<Vec<u8>>()
                ))
            }
            Err(_) => None,
        }
    });

    let body = Body::from_stream(anthropic_stream);
    Ok((StatusCode::OK, [("content-type", "text/event-stream")], body).into_response())
}

pub async fn from_openai_response(response: reqwest::Response) -> Result<Response, ProxyError> {
    let resp: OpenAIChatResponse = response
        .json()
        .await
        .map_err(|_| ProxyError::Internal("Failed to parse response".to_string()))?;

    Ok(Json(AnthropicChatResponse::from(&resp)).into_response())
}
