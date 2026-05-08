use axum::{
    Json, response::Response, http::StatusCode, 
    body::Body, response::IntoResponse
};
use futures::StreamExt;

use aidapter::{
    anthropic::prefix::{AnthropicChatResponse,AnthropicStreamEvent, AnthropicStreamChunk},
    openai::prefix::{OpenAIChatResponse,OpenAIStreamChunk},
};
use crate::error::ProxyError;

pub async fn from_anthropic_streaming(
    response: reqwest::Response,
) -> Result<Response, ProxyError> {
    use eventsource_stream::Eventsource;

    let byte_stream = response.bytes_stream();
    let event_stream = byte_stream.eventsource();

    let openai_stream = event_stream.filter_map(|result| async move {
        match result {
            Ok(event) => {
                if event.data == "[DONE]" {
                    return None;
                }

                let event: AnthropicStreamEvent = match serde_json::from_str(&event.data) {
                    Ok(event) => event,
                    Err(_) => return None,
                };

                let chunk_id = "chatcmpl-anthropic";
                let model = "claude";
                let chunks = Vec::<OpenAIStreamChunk>::from(&AnthropicStreamChunk {
                    id: chunk_id.to_string(),
                    model: model.to_string(),
                    event: event,
                });
                
                Some(Ok::<_, std::io::Error>(
                    chunks.into_iter().flat_map(|s| Vec::<u8>::from(&s)).collect::<Vec<u8>>()
                ))
            }
            Err(_) => None,
        }
    });

    let body = Body::from_stream(openai_stream);
    Ok((StatusCode::OK, [("content-type", "text/event-stream")], body).into_response())
}

pub async fn from_anthropic_response(response: reqwest::Response) -> Result<Response, ProxyError> {
    let resp: AnthropicChatResponse  = response
        .json()
        .await
        .map_err(|_| ProxyError::Internal("Failed to parse response".to_string()))?;

    Ok(Json(OpenAIChatResponse::from(&resp)).into_response())
}
