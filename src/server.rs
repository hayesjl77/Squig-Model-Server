use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::api::devtools::RequestLogger;
use crate::api::optimize::LiveInferenceSettings;
use crate::config::ServerConfig;
use crate::inference::InferenceManager;
use crate::models::{HfClient, ModelRegistry};

/// Shared application state available to all route handlers
#[derive(Clone)]
pub struct AppState {
    pub config: ServerConfig,
    pub model_registry: Arc<ModelRegistry>,
    pub inference_manager: Arc<InferenceManager>,
    pub hf_client: Arc<HfClient>,
    pub request_logger: Arc<RequestLogger>,
    pub live_inference: Arc<LiveInferenceSettings>,
    pub start_time: chrono::DateTime<chrono::Utc>,
}

pub async fn run(config: ServerConfig, open_browser: bool) -> Result<()> {
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);

    // Initialize model registry - scans for available models
    let model_registry = Arc::new(ModelRegistry::new(&config.models).await?);
    tracing::info!(
        "Found {} model(s) in {} directory(ies)",
        model_registry.available_models().len(),
        config.models.directories.len()
    );

    // Initialize inference manager - manages llama.cpp processes
    let inference_manager = Arc::new(InferenceManager::new(&config.inference).await?);

    // Initialize HuggingFace client
    let hf_client = Arc::new(HfClient::new());

    // Initialize request logger for developer tools
    let request_logger = Arc::new(RequestLogger::new());

    // Initialize live inference settings (mutable copy of config)
    let live_inference = Arc::new(LiveInferenceSettings::new(config.inference.clone()));

    // Auto-load default model if specified
    if !config.models.default_model.is_empty() {
        if let Some(model) = model_registry.find_model(&config.models.default_model) {
            tracing::info!("Loading default model: {}", model.name);
            let startup_settings = live_inference.read().clone();
            if let Err(e) = inference_manager
                .load_model(model.clone(), &startup_settings)
                .await
            {
                tracing::error!("Failed to load default model: {}", e);
            }
        } else {
            tracing::warn!(
                "Default model '{}' not found in model directories",
                config.models.default_model
            );
        }
    }

    let state = AppState {
        config: config.clone(),
        model_registry,
        inference_manager,
        hf_client,
        request_logger,
        live_inference,
        start_time: chrono::Utc::now(),
    };

    // Build router
    let app = Router::new()
        // OpenAI-compatible API routes
        .nest("/v1", crate::api::routes())
        // Server management API
        .nest("/api", crate::api::management_routes())
        // Dashboard UI (embedded static files)
        .fallback(crate::ui::static_handler)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Open browser
    if open_browser {
        let url = format!("http://{}", bind_addr);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = open::that(&url);
        });
    }

    tracing::info!("Server listening on {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
