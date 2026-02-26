pub mod chat;
pub mod completions;
pub mod devtools;
pub mod embeddings;
pub mod health;
pub mod huggingface;
pub mod models;
pub mod management;
pub mod optimize;
pub mod web_search;

use axum::{
    Router,
    routing::{get, post},
};

use crate::server::AppState;

/// OpenAI-compatible API routes mounted at /v1
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/chat/completions", post(chat::chat_completions))
        .route("/completions", post(completions::completions))
        .route("/embeddings", post(embeddings::embeddings))
        .route("/models", get(models::list_models))
        .route("/models/{model_id}", get(models::get_model))
}

/// Server management API routes mounted at /api
pub fn management_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/status", get(management::server_status))
        .route("/hardware", get(management::hardware_info))
        .route("/gpu-stats", get(management::gpu_stats))
        .route("/server-config", get(management::server_config))
        .route("/server/unload-all", post(management::unload_all))
        .route("/server/rescan-models", post(management::rescan_models))
        .route("/models/loaded", get(management::loaded_models))
        .route("/models/load", post(management::load_model))
        .route("/models/unload", post(management::unload_model))
        .route("/models/available", get(management::available_models))
        .route("/models/delete", post(management::delete_model))
        .route("/metrics", get(management::metrics))
        // HuggingFace integration
        .route("/hf/search", post(huggingface::hf_search))
        .route("/hf/download", post(huggingface::hf_download))
        .route("/hf/download-and-load", post(huggingface::hf_download_and_load))
        .route("/hf/downloads", get(huggingface::hf_downloads))
        .route("/hf/cancel", post(huggingface::hf_cancel))
        .route("/hf/clear", post(huggingface::hf_clear))
        // Developer tools
        .route("/dev/logs", get(devtools::api_logs))
        .route("/dev/logs/clear", post(devtools::clear_logs))
        .route("/dev/perf", get(devtools::perf_analysis))
        .route("/dev/perf/samples", get(devtools::perf_samples))
        // Self-optimization
        .route("/dev/optimize", post(optimize::self_optimize))
        .route("/dev/settings", get(optimize::get_settings))
        .route("/dev/apply-settings", post(optimize::apply_settings))
        // Web search
        .route("/web-search", post(web_search::web_search))
}
