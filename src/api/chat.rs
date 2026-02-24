use axum::{
    extract::State,
    response::{
        sse::Sse,
        IntoResponse, Json,
    },
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    // Extended sampling parameters (llama.cpp native)
    #[serde(default)]
    pub top_k: Option<i32>,
    #[serde(default)]
    pub min_p: Option<f32>,
    #[serde(default)]
    pub repeat_penalty: Option<f32>,
    #[serde(default)]
    pub repeat_last_n: Option<i32>,
    #[serde(default)]
    pub typical_p: Option<f32>,
    #[serde(default)]
    pub mirostat: Option<i32>,
    #[serde(default)]
    pub mirostat_tau: Option<f32>,
    #[serde(default)]
    pub mirostat_eta: Option<f32>,
    #[serde(default)]
    pub grammar: Option<String>,
    #[serde(default)]
    pub response_format: Option<serde_json::Value>,
    #[serde(default)]
    pub dynatemp_range: Option<f32>,
    #[serde(default)]
    pub dynatemp_exponent: Option<f32>,
    // Tool/function calling (OpenAI-compatible)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
}

/// OpenAI-compatible tool definition
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Tool call in assistant message
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Streaming tool call delta (partial tool call during streaming)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub call_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCallDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    /// Content can be null for assistant messages that only have tool_calls
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool calls made by the assistant
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// ID of the tool call this message is responding to (role=tool)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Function name when role=tool
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
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
        "messages": request.messages.iter().map(|m| {
            let content = m.content.as_deref().unwrap_or("");
            format!("[{}] {}...", m.role, &content[..content.len().min(100)])
        }).collect::<Vec<_>>(),
        "temperature": request.temperature,
        "max_tokens": request.max_tokens,
        "stream": is_stream,
        "tools": request.tools.is_some(),
    })).ok();

    if is_stream {
        // Forward streaming request to llama.cpp backend
        let logger = state.request_logger.clone();
        let model_for_log = model_name.clone();
        let msg_count = request.messages.len();

        match backend.chat_completions_stream(&request).await {
            Ok(inner_stream) => {
                // Wrap the stream to count tokens and log metrics after [DONE]
                let completion_tokens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let first_token_time: Arc<std::sync::Mutex<Option<std::time::Instant>>> =
                    Arc::new(std::sync::Mutex::new(None));
                let req_body_for_log = request_body_str.clone();

                let ct = completion_tokens.clone();
                let ftt = first_token_time.clone();

                let wrapped = inner_stream.map(move |event_result| {
                    // Each event from llama.cpp is ~1 token (or [DONE])
                    ct.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    // Record first token time
                    let mut ftt_lock = ftt.lock().unwrap();
                    if ftt_lock.is_none() {
                        *ftt_lock = Some(std::time::Instant::now());
                    }
                    event_result
                });

                // Use async_stream to forward events, then log on completion
                let logger_for_done = logger.clone();
                let model_for_done = model_for_log.clone();
                let ct_done = completion_tokens.clone();
                let ftt_done = first_token_time.clone();
                let stream_start = request_start;

                let logging_stream = async_stream::stream! {
                    tokio::pin!(wrapped);
                    while let Some(event_result) = wrapped.next().await {
                        yield event_result;
                    }

                    // Stream ended - log the completed request
                    // Subtract 1 from count because [DONE] event was also counted
                    let raw_count = ct_done.load(std::sync::atomic::Ordering::Relaxed);
                    let tokens = if raw_count > 0 { raw_count - 1 } else { 0 };

                    let duration_ms = stream_start.elapsed().as_millis() as u64;
                    let ttft = ftt_done.lock().unwrap()
                        .and_then(|t| t.checked_duration_since(stream_start))
                        .map(|d| d.as_millis() as u64);
                    let tps = if duration_ms > 0 && tokens > 0 {
                        tokens as f64 / duration_ms as f64 * 1000.0
                    } else {
                        0.0
                    };

                    logger_for_done.log_request(RequestLogEntry {
                        id: logger_for_done.next_id(),
                        timestamp: now_iso(),
                        method: "POST".to_string(),
                        path: "/v1/chat/completions".to_string(),
                        model: model_for_done,
                        request_summary: format!("Stream chat: {} message(s)", msg_count),
                        response_summary: format!("{} tokens in {}ms ({:.1} t/s)", tokens, duration_ms, tps),
                        status_code: 200,
                        duration_ms,
                        prompt_tokens: 0, // Not available from streaming
                        completion_tokens: tokens,
                        tokens_per_second: tps,
                        time_to_first_token_ms: ttft,
                        request_body: req_body_for_log,
                        response_body: None,
                    });
                };

                Ok(Sse::new(logging_stream).into_response())
            }
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
                        if let Some(ref tool_calls) = c.message.tool_calls {
                            format!("[tool_calls: {}]", tool_calls.iter().map(|tc| tc.function.name.clone()).collect::<Vec<_>>().join(", "))
                        } else {
                            let content = c.message.content.as_deref().unwrap_or("");
                            if content.len() > 200 {
                                format!("{}...", &content[..200])
                            } else {
                                content.to_string()
                            }
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
