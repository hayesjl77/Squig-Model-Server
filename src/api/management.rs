use axum::{extract::State, Json};
use serde::Deserialize;

use crate::inference::detect_hardware;
use crate::server::AppState;

/// Live GPU stats from nvidia-smi / rocm-smi
#[derive(serde::Serialize, Default)]
pub struct GpuLiveStats {
    pub index: usize,
    pub name: String,
    pub gpu_utilization_pct: f64,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_utilization_pct: f64,
    pub temperature_c: Option<f64>,
    pub power_draw_w: Option<f64>,
    pub power_limit_w: Option<f64>,
    pub fan_speed_pct: Option<f64>,
    pub clock_graphics_mhz: Option<u64>,
    pub clock_memory_mhz: Option<u64>,
}

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
            "gpu_backend": live.gpu_backend,
        },
        "loaded_models": loaded,
        "available_models_count": state.model_registry.available_models().len(),
        "metrics": metrics,
        "available_backends": state.inference_manager.available_backends(),
        "config": {
            "parallel_slots": live.parallel_slots,
            "context_size": live.context_size,
            "flash_attention": live.flash_attention,
            "gpu_layers": live.gpu_layers,
            "kv_cache_type_k": live.kv_cache_type_k,
            "kv_cache_type_v": live.kv_cache_type_v,
        }
    }))
}

/// GET /api/hardware - Hardware detection info
pub async fn hardware_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    let hw = detect_hardware();
    let live = state.live_inference.read().clone();
    let mut val = serde_json::json!(hw);
    // Override with the actual configured backend, not the auto-detected recommendation
    val["active_backend"] = serde_json::json!(live.gpu_backend);
    val["available_backends"] = serde_json::json!(state.inference_manager.available_backends());
    Json(val)
}

/// GET /api/gpu-stats - Live GPU utilization, VRAM, temp, power, clocks
pub async fn gpu_stats() -> Json<serde_json::Value> {
    let stats = collect_gpu_stats();
    Json(serde_json::json!({ "gpus": stats }))
}

fn collect_gpu_stats() -> Vec<GpuLiveStats> {
    let mut results = Vec::new();

    // Try NVIDIA GPUs via nvidia-smi
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,power.limit,fan.speed,clocks.current.graphics,clocks.current.memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let p: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if p.len() >= 11 {
                    let mem_used: u64 = p[3].parse().unwrap_or(0);
                    let mem_total: u64 = p[4].parse().unwrap_or(1);
                    let mem_pct = if mem_total > 0 {
                        (mem_used as f64 / mem_total as f64) * 100.0
                    } else {
                        0.0
                    };
                    results.push(GpuLiveStats {
                        index: p[0].parse().unwrap_or(0),
                        name: p[1].to_string(),
                        gpu_utilization_pct: p[2].parse().unwrap_or(0.0),
                        memory_used_mb: mem_used,
                        memory_total_mb: mem_total,
                        memory_utilization_pct: (mem_pct * 10.0).round() / 10.0,
                        temperature_c: p[5].parse().ok(),
                        power_draw_w: p[6].parse().ok(),
                        power_limit_w: p[7].parse().ok(),
                        fan_speed_pct: p[8].parse().ok(),
                        clock_graphics_mhz: p[9].parse().ok(),
                        clock_memory_mhz: p[10].parse().ok(),
                    });
                }
            }
        }
    }

    // Try AMD GPUs via rocm-smi
    if results.is_empty() {
        if let Ok(output) = std::process::Command::new("rocm-smi")
            .args(["--showuse", "--showmemuse", "--showtemp", "--showpower", "--showfan", "--csv"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // rocm-smi CSV parsing is more complex; fallback to simpler queries
                tracing::debug!("rocm-smi output: {}", stdout);
                // TODO: parse AMD GPU stats from rocm-smi CSV
            }
        }
    }

    results
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
            let mut obj = serde_json::json!({
                "name": m.name,
                "path": m.path.to_string_lossy(),
                "size_bytes": m.size_bytes,
                "size_human": format_bytes(m.size_bytes),
                "quantization": m.quantization,
                "parameters": m.parameters,
                "family": m.family,
            });
            if let Some(ref split) = m.split_info {
                obj.as_object_mut().unwrap().insert("split_info".into(), serde_json::json!({
                    "total_parts": split.total_parts,
                    "present_parts": split.present_parts,
                    "complete": split.complete,
                    "total_size_human": format_bytes(split.total_size_bytes),
                }));
            }
            obj
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

/// GET /api/server-config - Return current server configuration (port, host, model dirs, etc.)
pub async fn server_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    let loaded = state.inference_manager.loaded_models().await;
    let live = state.live_inference.read().clone();
    Json(serde_json::json!({
        "server": {
            "host": state.config.server.host,
            "port": state.config.server.port,
            "max_concurrent_requests": state.config.server.max_concurrent_requests,
            "api_key_set": !state.config.server.api_key.is_empty(),
        },
        "models": {
            "directories": state.config.models.directories,
            "default_model": state.config.models.default_model,
            "max_loaded_models": state.config.models.max_loaded_models,
        },
        "inference_engine": {
            "running": !loaded.is_empty(),
            "loaded_count": loaded.len(),
            "loaded_models": loaded,
            "gpu_backend": live.gpu_backend,
        },
    }))
}

/// POST /api/server/unload-all - Unload all models (turns inference off)
pub async fn unload_all(State(state): State<AppState>) -> Json<serde_json::Value> {
    let loaded = state.inference_manager.loaded_models().await;
    let mut unloaded = Vec::new();
    let mut errors = Vec::new();

    for model_name in loaded {
        match state.inference_manager.unload_model(&model_name).await {
            Ok(()) => unloaded.push(model_name),
            Err(e) => errors.push(format!("{}: {}", model_name, e)),
        }
    }

    Json(serde_json::json!({
        "status": if errors.is_empty() { "ok" } else { "partial" },
        "unloaded": unloaded,
        "errors": errors,
    }))
}

/// POST /api/server/rescan-models - Re-scan model directories
pub async fn rescan_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.model_registry.scan().await {
        Ok(()) => {
            let count = state.model_registry.available_models().len();
            Json(serde_json::json!({
                "status": "ok",
                "models_found": count,
            }))
        }
        Err(e) => Json(serde_json::json!({
            "error": format!("Rescan failed: {}", e)
        })),
    }
}

#[derive(Deserialize)]
pub struct DeleteModelRequest {
    pub model: String,
}

/// POST /api/models/delete - Delete a model file from disk (unloads first if loaded)
pub async fn delete_model(
    State(state): State<AppState>,
    Json(request): Json<DeleteModelRequest>,
) -> Json<serde_json::Value> {
    // Use exact match only for delete operations to prevent accidentally deleting wrong models
    let model = match state.model_registry.find_model_exact(&request.model) {
        Some(m) => m,
        None => {
            return Json(serde_json::json!({
                "error": format!("Model '{}' not found", request.model)
            }));
        }
    };

    // Unload if currently loaded
    let loaded = state.inference_manager.loaded_models().await;
    if loaded.iter().any(|n| n == &model.name) {
        if let Err(e) = state.inference_manager.unload_model(&model.name).await {
            tracing::warn!("Failed to unload model before delete: {}", e);
        }
    }

    // Get all file paths to delete (handles split/multi-shard models)
    let paths = state.model_registry.get_split_shard_paths(&model);
    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for path in &paths {
        match std::fs::remove_file(path) {
            Ok(()) => {
                tracing::info!("Deleted model file: {:?}", path);
                deleted.push(path.to_string_lossy().to_string());
            }
            Err(e) => {
                tracing::warn!("Failed to delete {:?}: {}", path, e);
                errors.push(format!("{}: {}", path.display(), e));
            }
        }
    }

    // Rescan so the registry is up to date
    let _ = state.model_registry.scan().await;

    if !deleted.is_empty() {
        Json(serde_json::json!({
            "status": "deleted",
            "model": model.name,
            "files_deleted": deleted.len(),
            "errors": errors,
        }))
    } else {
        Json(serde_json::json!({
            "error": format!("Failed to delete any files: {}", errors.join(", "))
        }))
    }
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
