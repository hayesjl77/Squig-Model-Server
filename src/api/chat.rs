use axum::{
    extract::State,
    response::{
        sse::Sse,
        IntoResponse, Json,
    },
};
use serde::{Deserialize, Serialize};

use crate::api::devtools::{now_iso, RequestLogEntry};
use crate::server::AppState;

/// OpenAI-compatible chat completion request
#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<usize>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    #[serde(default)]
    pub frequency_penalty: Option<f32>,
    #[serde(default)]
    pub seed: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI-compatible chat completion response
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// Streaming chunk
#[derive(Debug, Serialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChunkChoice>,
}

#[derive(Debug, Serialize)]
pub struct ChatChunkChoice {
    pub index: usize,
    pub delta: ChatDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// POST /v1/chat/completions
pub async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    let is_stream = request.stream.unwrap_or(false);
    let model_name = request.model.clone();

    // Find active inference backend for this model
    let backend = match state.inference_manager.get_backend(&model_name).await {
        Some(b) => b,
        None => {
            // Try to find and auto-load the model
            if let Some(model_info) = state.model_registry.find_model(&model_name) {
                let live_settings = state.live_inference.read().clone();
                match state
                    .inference_manager
                    .load_model(model_info.clone(), &live_settings)
                    .await
                {
                    Ok(()) => match state.inference_manager.get_backend(&model_name).await {
                        Some(b) => b,
                        None => {
                            return Err(Json(serde_json::json!({
                                "error": {
                                    "message": "Model loaded but backend unavailable",
                                    "type": "server_error",
                                    "code": "model_error"
                                }
                            })));
                        }
                    },
                    Err(e) => {
                        return Err(Json(serde_json::json!({
                            "error": {
                                "message": format!("Failed to load model: {}", e),
                                "type": "server_error",
                                "code": "model_load_error"
                            }
                        })));
                    }
                }
            } else {
                return Err(Json(serde_json::json!({
                    "error": {
                        "message": format!("Model '{}' not found", model_name),
                        "type": "invalid_request_error",
                        "code": "model_not_found"
                    }
                })));
            }
        }
    };

    let request_start = std::time::Instant::now();
    let request_body_str = serde_json::to_string(&serde_json::json!({
        "model": &request.model,
        "messages": request.messages.iter().map(|m| format!("[{}] {}...", m.role, &m.content[..m.content.len().min(100)])).collect::<Vec<_>>(),
        "temperature": request.temperature,
        "max_tokens": request.max_tokens,
        "stream": is_stream,
    })).ok();

    if is_stream {
        // Forward streaming request to llama.cpp backend
        let logger = state.request_logger.clone();
        let log_id = logger.next_id();
        let model_for_log = model_name.clone();

        // Log the start of a streaming request
        logger.log_request(RequestLogEntry {
            id: log_id,
            timestamp: now_iso(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            model: model_for_log,
            request_summary: format!("Stream chat: {} message(s)", request.messages.len()),
            response_summary: "Streaming...".to_string(),
            status_code: 200,
            duration_ms: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            tokens_per_second: 0.0,
            time_to_first_token_ms: None,
            request_body: request_body_str.clone(),
            response_body: None,
        });

        match backend.chat_completions_stream(&request).await {
            Ok(stream) => Ok(Sse::new(stream).into_response()),
            Err(e) => Err(Json(serde_json::json!({
                "error": {
                    "message": format!("Inference error: {}", e),
                    "type": "server_error",
                    "code": "inference_error"
                }
            }))),
        }
    } else {
        // Forward non-streaming request
        match backend.chat_completions(&request).await {
            Ok(response) => {
                let duration_ms = request_start.elapsed().as_millis() as u64;
                let completion_tokens = response.usage.completion_tokens;
                let prompt_tokens = response.usage.prompt_tokens;
                let tps = if duration_ms > 0 {
                    completion_tokens as f64 / duration_ms as f64 * 1000.0
                } else {
                    0.0
                };

                let response_preview = response.choices.first()
                    .map(|c| {
                        let content = &c.message.content;
                        if content.len() > 200 {
                            format!("{}...", &content[..200])
                        } else {
                            content.clone()
                        }
                    })
                    .unwrap_or_default();

                state.request_logger.log_request(RequestLogEntry {
                    id: state.request_logger.next_id(),
                    timestamp: now_iso(),
                    method: "POST".to_string(),
                    path: "/v1/chat/completions".to_string(),
                    model: model_name.clone(),
                    request_summary: format!("Chat: {} message(s), {} prompt tokens", request.messages.len(), prompt_tokens),
                    response_summary: format!("{} tokens in {}ms ({:.1} t/s)", completion_tokens, duration_ms, tps),
                    status_code: 200,
                    duration_ms,
                    prompt_tokens,
                    completion_tokens,
                    tokens_per_second: tps,
                    time_to_first_token_ms: None,
                    request_body: request_body_str,
                    response_body: Some(response_preview),
                });

                Ok(Json(response).into_response())
            }
            Err(e) => {
                let duration_ms = request_start.elapsed().as_millis() as u64;
                state.request_logger.log_request(RequestLogEntry {
                    id: state.request_logger.next_id(),
                    timestamp: now_iso(),
                    method: "POST".to_string(),
                    path: "/v1/chat/completions".to_string(),
                    model: model_name.clone(),
                    request_summary: format!("Chat: {} message(s)", request.messages.len()),
                    response_summary: format!("ERROR: {}", e),
                    status_code: 500,
                    duration_ms,
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    tokens_per_second: 0.0,
                    time_to_first_token_ms: None,
                    request_body: request_body_str,
                    response_body: Some(format!("Error: {}", e)),
                });

                Err(Json(serde_json::json!({
                    "error": {
                        "message": format!("Inference error: {}", e),
                        "type": "server_error",
                        "code": "inference_error"
                    }
                })))
            }
        }
    }
}
