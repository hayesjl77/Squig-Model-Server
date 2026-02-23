use axum::{extract::State, Json};
use serde::Deserialize;

use crate::inference::detect_hardware;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct UnloadModelRequest {
    pub model: String,
}

/// GET /api/status - Full server status for dashboard
pub async fn server_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let uptime = chrono::Utc::now() - state.start_time;
    let hw = detect_hardware();
    let loaded = state.inference_manager.loaded_models().await;
    let metrics = state.inference_manager.get_metrics().await;

    let live = state.live_inference.read().clone();

    Json(serde_json::json!({
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": uptime.num_seconds(),
        "hardware": {
            "cpu": hw.cpu_name,
            "cpu_cores": hw.cpu_cores,
            "total_memory_gb": hw.total_memory_gb,
            "available_memory_gb": hw.available_memory_gb,
            "gpus": hw.gpus,
            "gpu_backend": hw.recommended_backend,
        },
        "loaded_models": loaded,
        "available_models_count": state.model_registry.available_models().len(),
        "metrics": metrics,
        "config": {
            "parallel_slots": live.parallel_slots,
            "context_size": live.context_size,
            "flash_attention": live.flash_attention,
            "gpu_layers": live.gpu_layers,
            "kv_cache_type": live.kv_cache_type,
        }
    }))
}

/// GET /api/hardware - Hardware detection info
pub async fn hardware_info() -> Json<serde_json::Value> {
    let hw = detect_hardware();
    Json(serde_json::json!(hw))
}

/// GET /api/models/loaded - Currently loaded models with stats
pub async fn loaded_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    let loaded = state.inference_manager.loaded_model_details().await;
    Json(serde_json::json!({ "models": loaded }))
}

/// GET /api/models/available - All discovered models
pub async fn available_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    let models: Vec<_> = state
        .model_registry
        .available_models()
        .iter()
        .map(|m| {
            serde_json::json!({
                "name": m.name,
                "path": m.path.to_string_lossy(),
                "size_bytes": m.size_bytes,
                "size_human": format_bytes(m.size_bytes),
                "quantization": m.quantization,
                "parameters": m.parameters,
                "family": m.family,
            })
        })
        .collect();

    Json(serde_json::json!({ "models": models }))
}

#[derive(Deserialize)]
pub struct LoadModelRequest {
    pub model: String,
}

/// POST /api/models/load - Load a model into memory
pub async fn load_model(
    State(state): State<AppState>,
    Json(request): Json<LoadModelRequest>,
) -> Json<serde_json::Value> {
    let model = match state.model_registry.find_model(&request.model) {
        Some(m) => m.clone(),
        None => {
            return Json(serde_json::json!({
                "error": format!("Model '{}' not found", request.model)
            }));
        }
    };

    let live_settings = state.live_inference.read().clone();
    match state
        .inference_manager
        .load_model(model.clone(), &live_settings)
        .await
    {
        Ok(()) => Json(serde_json::json!({
            "status": "loaded",
            "model": model.name,
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Failed to load model: {}", e)
        })),
    }
}

/// POST /api/models/unload - Unload a model from memory
pub async fn unload_model(
    State(state): State<AppState>,
    Json(request): Json<UnloadModelRequest>,
) -> Json<serde_json::Value> {
    match state.inference_manager.unload_model(&request.model).await {
        Ok(()) => Json(serde_json::json!({
            "status": "unloaded",
            "model": request.model,
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Failed to unload model: {}", e)
        })),
    }
}

/// GET /api/metrics - Throughput and performance metrics
pub async fn metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let metrics = state.inference_manager.get_metrics().await;
    Json(serde_json::json!(metrics))
}

fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    }
}
