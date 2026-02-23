use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::server::AppState;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
}

/// POST /api/hf/search - Search HuggingFace for GGUF models
pub async fn hf_search(
    State(state): State<AppState>,
    Json(query): Json<SearchQuery>,
) -> Json<serde_json::Value> {
    let limit = query.limit.unwrap_or(20);

    match state.hf_client.search(&query.q, limit).await {
        Ok(results) => Json(serde_json::json!({
            "query": query.q,
            "count": results.len(),
            "results": results,
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Search failed: {}", e)
        })),
    }
}

#[derive(Deserialize)]
pub struct DownloadRequest {
    pub repo_id: String,
    pub filename: String,
}

/// POST /api/hf/download - Start downloading a GGUF model from HuggingFace
pub async fn hf_download(
    State(state): State<AppState>,
    Json(req): Json<DownloadRequest>,
) -> Json<serde_json::Value> {
    // Download to the first configured models directory
    let dest_dir = state
        .config
        .models
        .directories
        .first()
        .cloned()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".squig-models"))
                .unwrap_or_else(|| std::path::PathBuf::from("./models"))
        });

    match state.hf_client.start_download(&req.repo_id, &req.filename, &dest_dir) {
        Ok(()) => Json(serde_json::json!({
            "status": "started",
            "repo_id": req.repo_id,
            "filename": req.filename,
            "destination": dest_dir.to_string_lossy(),
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// GET /api/hf/downloads - Get progress of all active downloads
pub async fn hf_downloads(State(state): State<AppState>) -> Json<serde_json::Value> {
    let downloads = state.hf_client.download_progress();
    Json(serde_json::json!({
        "downloads": downloads,
    }))
}

#[derive(Deserialize)]
pub struct CancelRequest {
    pub repo_id: String,
    pub filename: String,
}

/// POST /api/hf/cancel - Cancel an active download
pub async fn hf_cancel(
    State(state): State<AppState>,
    Json(req): Json<CancelRequest>,
) -> Json<serde_json::Value> {
    match state.hf_client.cancel_download(&req.repo_id, &req.filename) {
        Ok(()) => Json(serde_json::json!({
            "status": "cancelling",
            "repo_id": req.repo_id,
            "filename": req.filename,
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// POST /api/hf/clear - Clear completed/failed download entries
pub async fn hf_clear(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.hf_client.clear_finished_downloads();
    Json(serde_json::json!({ "status": "cleared" }))
}

/// POST /api/hf/download-and-load - Download a GGUF file then auto-load it when complete
pub async fn hf_download_and_load(
    State(state): State<AppState>,
    Json(req): Json<DownloadRequest>,
) -> Json<serde_json::Value> {
    let dest_dir = state
        .config
        .models
        .directories
        .first()
        .cloned()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".squig-models"))
                .unwrap_or_else(|| std::path::PathBuf::from("./models"))
        });

    // Start the download
    if let Err(e) = state.hf_client.start_download(&req.repo_id, &req.filename, &dest_dir) {
        return Json(serde_json::json!({
            "error": format!("{}", e)
        }));
    }

    // Spawn a task that watches for completion, then re-scans models and auto-loads
    let filename = req.filename.clone();
    let repo_id = req.repo_id.clone();
    let registry = Arc::clone(&state.model_registry);
    let manager = Arc::clone(&state.inference_manager);
    let inference_config = state.config.inference.clone();
    let hf_client = Arc::clone(&state.hf_client);

    tokio::spawn(async move {
        // Poll download progress until complete
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let progress = hf_client.download_progress();
            let dl = progress.iter().find(|d| d.repo_id == repo_id && d.filename == filename);
            match dl {
                Some(d) if d.status == crate::models::huggingface::DownloadStatus::Complete => {
                    break;
                }
                Some(d) if d.status == crate::models::huggingface::DownloadStatus::Failed => {
                    tracing::error!("Download-and-load failed for {}/{}", repo_id, filename);
                    return;
                }
                None => return, // Download entry disappeared
                _ => continue,
            }
        }

        // Re-scan model directories
        if let Err(e) = registry.scan().await {
            tracing::error!("Failed to rescan models after download: {}", e);
            return;
        }

        // Find and auto-load the downloaded model
        let stem = filename.strip_suffix(".gguf").unwrap_or(&filename);
        if let Some(model) = registry.find_model(stem) {
            tracing::info!("Auto-loading downloaded model: {}", model.name);
            if let Err(e) = manager.load_model(model, &inference_config).await {
                tracing::error!("Auto-load failed: {}", e);
            }
        } else {
            tracing::warn!("Downloaded model {} not found in registry after rescan", filename);
        }
    });

    Json(serde_json::json!({
        "status": "download_started",
        "auto_load": true,
        "repo_id": req.repo_id,
        "filename": req.filename,
    }))
}
