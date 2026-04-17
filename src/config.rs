use std::collections::HashMap;
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

    /// KV cache quantization type for Keys: "f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"
    pub kv_cache_type_k: String,

    /// KV cache quantization type for Values: "f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"
    pub kv_cache_type_v: String,

    /// CPU threads for generation (-1 = auto)
    #[serde(default = "default_neg_one_i32")]
    pub threads: i32,

    /// CPU threads for batch/prompt processing (-1 = same as threads)
    #[serde(default = "default_neg_one_i32")]
    pub threads_batch: i32,

    /// Logical batch size for prompt processing (default: 2048)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Physical batch size (default: 512)
    #[serde(default = "default_ubatch_size")]
    pub ubatch_size: usize,

    /// Force model to stay in RAM (no swap)
    #[serde(default)]
    pub mlock: bool,

    /// Disable memory-mapped model loading
    #[serde(default)]
    pub no_mmap: bool,

    /// Default max tokens to predict per request (-1 = infinity)
    #[serde(default = "default_neg_one_i32")]
    pub n_predict: i32,

    /// RoPE frequency scaling method: "", "none", "linear", "yarn"
    #[serde(default)]
    pub rope_scaling: String,

    /// RoPE base frequency (0.0 = use model default)
    #[serde(default)]
    pub rope_freq_base: f64,

    /// RoPE frequency scale (0.0 = use model default)
    #[serde(default)]
    pub rope_freq_scale: f64,

    /// Multi-GPU split mode: "none", "layer", "row"
    #[serde(default = "default_split_mode")]
    pub split_mode: String,

    /// Main GPU index for split_mode=none or KV with split_mode=row
    #[serde(default)]
    pub main_gpu: i32,

    /// Tensor split ratios across GPUs (comma-separated, e.g. "3,1")
    #[serde(default)]
    pub tensor_split: String,

    /// Enable prompt caching (reuse KV cache across requests)
    #[serde(default = "default_true")]
    pub cache_prompt: bool,

    /// Perform warmup run on model load
    #[serde(default = "default_true")]
    pub warmup: bool,

    /// Enable smart defaults that auto-tune settings based on hardware/model size.
    /// When disabled, user settings are passed through unchanged.
    #[serde(default = "default_true")]
    pub smart_defaults: bool,

    /// Path to llama.cpp server binary (auto-detected if empty)
    /// DEPRECATED: Use backend_paths instead. Kept for backwards compatibility.
    #[serde(default)]
    pub llama_server_path: String,

    /// Per-backend paths to llama-server binaries
    /// Keys: "vulkan", "cuda", "rocm", "cpu"
    /// Values: absolute paths to the respective llama-server binary
    #[serde(default)]
    pub backend_paths: HashMap<String, String>,
}

fn default_neg_one_i32() -> i32 { -1 }
fn default_batch_size() -> usize { 2048 }
fn default_ubatch_size() -> usize { 512 }
fn default_split_mode() -> String { "layer".to_string() }
fn default_true() -> bool { true }

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
                host: "0.0.0.0".to_string(),
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
                kv_cache_type_k: "q8_0".to_string(),
                kv_cache_type_v: "q8_0".to_string(),
                threads: -1,
                threads_batch: -1,
                batch_size: 2048,
                ubatch_size: 512,
                mlock: false,
                no_mmap: false,
                n_predict: -1,
                rope_scaling: String::new(),
                rope_freq_base: 0.0,
                rope_freq_scale: 0.0,
                split_mode: "layer".to_string(),
                main_gpu: 0,
                tensor_split: String::new(),
                cache_prompt: true,
                warmup: true,
                smart_defaults: true,
                llama_server_path: String::new(),
                backend_paths: HashMap::new(),
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
