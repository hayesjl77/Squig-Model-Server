use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::server::AppState;

/// OpenAI-compatible embedding request
#[derive(Debug, Deserialize)]
pub struct EmbeddingRequest {
    /// The input text(s) to embed. Can be a single string or array of strings.
    pub input: EmbeddingInput,
    /// Model name (must be loaded)
    pub model: String,
    /// Optional encoding format — only "float" is supported
    #[serde(default)]
    pub encoding_format: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    Single(String),
    Multiple(Vec<String>),
}

impl EmbeddingInput {
    pub fn into_vec(self) -> Vec<String> {
        match self {
            EmbeddingInput::Single(s) => vec![s],
            EmbeddingInput::Multiple(v) => v,
        }
    }
}

/// OpenAI-compatible embedding response
#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: usize,
    pub total_tokens: usize,
}

/// POST /v1/embeddings
///
/// Proxies the request to the loaded llama-server's /v1/embeddings endpoint.
/// Requires that the llama-server was launched with --embedding flag.
pub async fn embeddings(
    State(state): State<AppState>,
    Json(request): Json<EmbeddingRequest>,
) -> impl IntoResponse {
    let backend = match state.inference_manager.get_backend(&request.model).await {
        Some(b) => b,
        None => {
            // If a specific model isn't found, try the first loaded model
            match state.inference_manager.get_any_backend().await {
                Some(b) => b,
                None => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({
                            "error": {
                                "message": format!("Model '{}' not loaded and no models available", request.model),
                                "type": "invalid_request_error",
                                "code": "model_not_found"
                            }
                        })),
                    )
                        .into_response();
                }
            }
        }
    };

    match backend.embeddings(&request).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "message": format!("Embedding error: {}", e),
                    "type": "server_error",
                    "code": "embedding_error"
                }
            })),
        )
            .into_response(),
    }
}
