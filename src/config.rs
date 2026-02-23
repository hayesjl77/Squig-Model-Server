use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Top-level server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: ServerSettings,
    pub models: ModelSettings,
    pub inference: InferenceSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
    /// Maximum concurrent requests across all models
    pub max_concurrent_requests: usize,
    /// API key for authentication (empty = no auth)
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    /// Directories to scan for GGUF model files
    pub directories: Vec<PathBuf>,
    /// Default model to load on startup (model filename or alias)
    pub default_model: String,
    /// Maximum models to keep loaded simultaneously
    pub max_loaded_models: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceSettings {
    /// Number of GPU layers to offload (-1 = all)
    pub gpu_layers: i32,
    /// Context size (tokens)
    pub context_size: usize,
    /// Number of parallel request slots per model
    pub parallel_slots: usize,
    /// Enable flash attention
    pub flash_attention: bool,
    /// Enable continuous batching
    pub continuous_batching: bool,
    /// GPU backend preference: "auto", "vulkan", "cuda", "rocm", "cpu"
    pub gpu_backend: String,

    /// Speculative decoding settings
    pub speculative: SpeculativeSettings,

    /// KV cache quantization type: "f16", "q8_0", "q4_0"
    pub kv_cache_type: String,

    /// Path to llama.cpp server binary (auto-detected if empty)
    pub llama_server_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeSettings {
    pub enabled: bool,
    /// Path to draft model for speculative decoding
    pub draft_model: String,
    /// Max tokens to draft
    pub draft_max: usize,
    /// Min tokens to draft
    pub draft_min: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        // Determine a sensible default models directory
        let default_models_dir = dirs::home_dir()
            .map(|h| h.join(".squig-models"))
            .unwrap_or_else(|| PathBuf::from("./models"));

        Self {
            server: ServerSettings {
                host: "127.0.0.1".to_string(),
                port: 9090,
                max_concurrent_requests: 16,
                api_key: String::new(),
            },
            models: ModelSettings {
                directories: vec![default_models_dir],
                default_model: String::new(),
                max_loaded_models: 2,
            },
            inference: InferenceSettings {
                gpu_layers: -1, // offload all layers
                context_size: 32768,
                parallel_slots: 4,
                flash_attention: true,
                continuous_batching: true,
                gpu_backend: "auto".to_string(),
                speculative: SpeculativeSettings {
                    enabled: false,
                    draft_model: String::new(),
                    draft_max: 16,
                    draft_min: 4,
                },
                kv_cache_type: "q8_0".to_string(),
                llama_server_path: String::new(),
            },
        }
    }
}

impl ServerConfig {
    pub fn load(path: &PathBuf) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: ServerConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            tracing::warn!("Config file not found at {:?}, using defaults", path);
            let config = Self::default();
            // Write default config for user reference
            let toml_str = toml::to_string_pretty(&config)?;
            if let Err(e) = std::fs::write(path, &toml_str) {
                tracing::warn!("Could not write default config: {}", e);
            } else {
                tracing::info!("Generated default config at {:?}", path);
            }
            Ok(config)
        }
    }
}
