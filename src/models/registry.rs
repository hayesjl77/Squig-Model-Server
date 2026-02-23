use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::config::ModelSettings;

/// Information about a discovered GGUF model file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Display name (derived from filename)
    pub name: String,
    /// Full path to the GGUF file
    pub path: PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
    /// Detected quantization (e.g., "Q4_K_M", "Q5_K_M")
    pub quantization: String,
    /// Estimated parameter count (e.g., "7B", "32B", "70B")
    pub parameters: String,
    /// Model family (e.g., "qwen2.5", "llama3", "deepseek")
    pub family: String,
    /// When this model was discovered
    pub discovered_at: DateTime<Utc>,
}

/// Scans configured directories for GGUF models and manages the catalog
pub struct ModelRegistry {
    models: RwLock<Vec<ModelInfo>>,
    directories: Vec<PathBuf>,
}

impl ModelRegistry {
    pub async fn new(settings: &ModelSettings) -> Result<Self> {
        let registry = Self {
            models: RwLock::new(Vec::new()),
            directories: settings.directories.clone(),
        };

        // Ensure model directories exist
        for dir in &registry.directories {
            if !dir.exists() {
                tracing::info!("Creating models directory: {:?}", dir);
                std::fs::create_dir_all(dir)?;
            }
        }

        // Scan for models
        registry.scan().await?;

        Ok(registry)
    }

    /// Scan all configured directories for GGUF files
    pub async fn scan(&self) -> Result<()> {
        let mut found = Vec::new();

        for dir in &self.directories {
            if !dir.exists() {
                continue;
            }

            tracing::debug!("Scanning {:?} for GGUF models...", dir);
            scan_directory(dir, &mut found)?;
        }

        found.sort_by(|a, b| a.name.cmp(&b.name));
        let count = found.len();
        *self.models.write() = found;

        tracing::info!("Model scan complete: {} model(s) found", count);
        Ok(())
    }

    /// Get all available models
    pub fn available_models(&self) -> Vec<ModelInfo> {
        self.models.read().clone()
    }

    /// Find a model by name (case-insensitive, supports partial match)
    pub fn find_model(&self, query: &str) -> Option<ModelInfo> {
        let models = self.models.read();
        let query_lower = query.to_lowercase();

        // Exact match first
        if let Some(m) = models.iter().find(|m| m.name.to_lowercase() == query_lower) {
            return Some(m.clone());
        }

        // Partial match (contains)
        if let Some(m) = models
            .iter()
            .find(|m| m.name.to_lowercase().contains(&query_lower))
        {
            return Some(m.clone());
        }

        // Match by filename stem
        models
            .iter()
            .find(|m| {
                m.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
            })
            .cloned()
    }
}

/// Recursively scan a directory for .gguf files
fn scan_directory(dir: &Path, models: &mut Vec<ModelInfo>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            scan_directory(&path, models)?;
        } else if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "gguf" {
                if let Some(model) = parse_gguf_model(&path) {
                    tracing::debug!("  Found: {} ({}, {})", model.name, model.parameters, model.quantization);
                    models.push(model);
                }
            }
        }
    }
    Ok(())
}

/// Parse a GGUF filename to extract model metadata
/// Common patterns:
///   qwen2.5-coder-32b-instruct-q5_k_m.gguf
///   Meta-Llama-3.1-70B-Instruct-Q4_K_M.gguf
///   deepseek-r1-distill-qwen-32b-Q5_K_M.gguf
fn parse_gguf_model(path: &Path) -> Option<ModelInfo> {
    let filename = path.file_stem()?.to_str()?;
    let metadata = std::fs::metadata(path).ok()?;

    let name = filename.to_string();
    let lower = filename.to_lowercase();

    // Extract quantization
    let quantization = extract_quantization(&lower);

    // Extract parameter count
    let parameters = extract_parameters(&lower);

    // Extract model family
    let family = extract_family(&lower);

    Some(ModelInfo {
        name,
        path: path.to_path_buf(),
        size_bytes: metadata.len(),
        quantization,
        parameters,
        family,
        discovered_at: Utc::now(),
    })
}

fn extract_quantization(filename: &str) -> String {
    let quant_patterns = [
        "q2_k", "q3_k_s", "q3_k_m", "q3_k_l", "q4_0", "q4_1", "q4_k_s", "q4_k_m",
        "q5_0", "q5_1", "q5_k_s", "q5_k_m", "q6_k", "q8_0", "f16", "f32",
        "iq1_s", "iq1_m", "iq2_xxs", "iq2_xs", "iq2_s", "iq2_m", "iq3_xxs", "iq3_xs",
        "iq3_s", "iq3_m", "iq4_xs", "iq4_nl",
    ];

    for pattern in &quant_patterns {
        if filename.contains(pattern) {
            return pattern.to_uppercase();
        }
    }
    "unknown".to_string()
}

fn extract_parameters(filename: &str) -> String {
    // Look for patterns like "7b", "13b", "32b", "70b", "0.5b"
    let param_patterns = [
        ("0.5b", "0.5B"),
        ("1.5b", "1.5B"),
        ("1b", "1B"),
        ("3b", "3B"),
        ("7b", "7B"),
        ("8b", "8B"),
        ("13b", "13B"),
        ("14b", "14B"),
        ("22b", "22B"),
        ("32b", "32B"),
        ("34b", "34B"),
        ("70b", "70B"),
        ("72b", "72B"),
        ("123b", "123B"),
        ("405b", "405B"),
    ];

    for (pattern, label) in &param_patterns {
        if filename.contains(pattern) {
            return label.to_string();
        }
    }
    "unknown".to_string()
}

fn extract_family(filename: &str) -> String {
    let families = [
        ("qwen2.5", "Qwen 2.5"),
        ("qwen2", "Qwen 2"),
        ("qwen3", "Qwen 3"),
        ("deepseek-r1", "DeepSeek R1"),
        ("deepseek-v3", "DeepSeek V3"),
        ("deepseek", "DeepSeek"),
        ("llama-3.3", "Llama 3.3"),
        ("llama-3.1", "Llama 3.1"),
        ("llama-3", "Llama 3"),
        ("llama3", "Llama 3"),
        ("meta-llama", "Llama"),
        ("mistral", "Mistral"),
        ("mixtral", "Mixtral"),
        ("phi-4", "Phi 4"),
        ("phi-3", "Phi 3"),
        ("gemma-2", "Gemma 2"),
        ("gemma", "Gemma"),
        ("yi-", "Yi"),
        ("internlm", "InternLM"),
        ("codellama", "Code Llama"),
        ("starcoder", "StarCoder"),
        ("command-r", "Command R"),
    ];

    for (pattern, label) in &families {
        if filename.contains(pattern) {
            return label.to_string();
        }
    }
    "Unknown".to_string()
}
