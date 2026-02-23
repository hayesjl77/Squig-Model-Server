use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::server::AppState;

#[derive(Debug, Serialize)]
pub struct ModelObject {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

#[derive(Debug, Serialize)]
pub struct ModelList {
    pub object: String,
    pub data: Vec<ModelObject>,
}

/// GET /v1/models - List available models (OpenAI-compatible)
pub async fn list_models(State(state): State<AppState>) -> Json<ModelList> {
    let mut models: Vec<ModelObject> = state
        .model_registry
        .available_models()
        .iter()
        .map(|m| ModelObject {
            id: m.name.clone(),
            object: "model".to_string(),
            created: m.discovered_at.timestamp(),
            owned_by: "local".to_string(),
        })
        .collect();

    // Also include currently-loaded models by their aliases
    for loaded in state.inference_manager.loaded_models().await {
        if !models.iter().any(|m| m.id == loaded) {
            models.push(ModelObject {
                id: loaded,
                object: "model".to_string(),
                created: state.start_time.timestamp(),
                owned_by: "local".to_string(),
            });
        }
    }

    Json(ModelList {
        object: "list".to_string(),
        data: models,
    })
}

/// GET /v1/models/:model_id
pub async fn get_model(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
) -> Json<serde_json::Value> {
    if let Some(model) = state.model_registry.find_model(&model_id) {
        Json(serde_json::json!({
            "id": model.name,
            "object": "model",
            "created": model.discovered_at.timestamp(),
            "owned_by": "local",
            "meta": {
                "path": model.path.to_string_lossy(),
                "size_bytes": model.size_bytes,
                "quantization": model.quantization,
                "parameters": model.parameters,
            }
        }))
    } else {
        Json(serde_json::json!({
            "error": {
                "message": format!("Model '{}' not found", model_id),
                "type": "invalid_request_error",
                "code": "model_not_found"
            }
        }))
    }
}
