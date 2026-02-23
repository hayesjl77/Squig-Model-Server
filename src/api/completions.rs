use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::server::AppState;

/// OpenAI-compatible text completion request
#[derive(Debug, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub max_tokens: Option<usize>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub seed: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<CompletionChoice>,
    pub usage: super::chat::Usage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionChoice {
    pub index: usize,
    pub text: String,
    pub finish_reason: Option<String>,
}

/// POST /v1/completions
pub async fn completions(
    State(state): State<AppState>,
    Json(request): Json<CompletionRequest>,
) -> impl IntoResponse {
    let backend = match state.inference_manager.get_backend(&request.model).await {
        Some(b) => b,
        None => {
            return Json(serde_json::json!({
                "error": {
                    "message": format!("Model '{}' not loaded", request.model),
                    "type": "invalid_request_error",
                    "code": "model_not_found"
                }
            }))
            .into_response();
        }
    };

    match backend.completions(&request).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => Json(serde_json::json!({
            "error": {
                "message": format!("Inference error: {}", e),
                "type": "server_error",
                "code": "inference_error"
            }
        }))
        .into_response(),
    }
}
