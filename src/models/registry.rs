use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::ModelSettings;

/// Information about a split GGUF model (multi-part file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitInfo {
    /// Total number of parts in the split set
    pub total_parts: u32,
    /// How many parts are actually present on disk
    pub present_parts: u32,
    /// Whether all parts are present and the model is loadable
    pub complete: bool,
    /// Combined size across all present shards
    pub total_size_bytes: u64,
}

/// Information about a discovered GGUF model file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Display name (derived from filename)
    pub name: String,
    /// Full path to the GGUF file (for splits, path to the first shard)
    pub path: PathBuf,
    /// File size in bytes (for splits, total across all present shards)
    pub size_bytes: u64,
    /// Detected quantization (e.g., "Q4_K_M", "Q5_K_M")
    pub quantization: String,
    /// Estimated parameter count (e.g., "7B", "32B", "70B")
    pub parameters: String,
    /// Model family (e.g., "qwen2.5", "llama3", "deepseek")
    pub family: String,
    /// When this model was discovered
    pub discovered_at: DateTime<Utc>,
    /// If this is a split/multi-part GGUF, details about the split
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_info: Option<SplitInfo>,
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

    /// Find a model by exact name match only (for destructive operations like delete)
    pub fn find_model_exact(&self, name: &str) -> Option<ModelInfo> {
        let models = self.models.read();
        let name_lower = name.to_lowercase();
        models.iter().find(|m| m.name.to_lowercase() == name_lower).cloned()
    }

    /// Get all file paths belonging to a split model group.
    /// For a split model with base name X, finds all files matching X-NNNNN-of-NNNNN.gguf
    /// in the same directory.
    pub fn get_split_shard_paths(&self, model: &ModelInfo) -> Vec<PathBuf> {
        if model.split_info.is_none() {
            return vec![model.path.clone()];
        }
        let parent = match model.path.parent() {
            Some(p) => p,
            None => return vec![model.path.clone()],
        };
        let base_lower = model.name.to_lowercase();
        let split_re = Regex::new(r"^(.+)-\d{5}-of-\d{5}\.gguf$").unwrap();
        let mut paths = Vec::new();
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_lowercase();
                if let Some(caps) = split_re.captures(&fname) {
                    if caps[1].to_lowercase() == base_lower {
                        paths.push(entry.path());
                    }
                }
            }
        }
        if paths.is_empty() {
            vec![model.path.clone()]
        } else {
            paths
        }
    }
}

/// Recursively scan a directory for .gguf files, grouping split shards
fn scan_directory(dir: &Path, models: &mut Vec<ModelInfo>) -> Result<()> {
    // First pass: collect all .gguf files
    let mut gguf_files: Vec<PathBuf> = Vec::new();
    collect_gguf_files(dir, &mut gguf_files)?;

    // Detect split files: pattern like "name-00001-of-00004.gguf"
    let split_re = Regex::new(r"^(.+)-(\d{5})-of-(\d{5})$").unwrap();

    // Group split files by their base name
    let mut split_groups: HashMap<String, Vec<(PathBuf, u32, u32)>> = HashMap::new(); // base -> [(path, part_num, total)]
    let mut standalone: Vec<PathBuf> = Vec::new();

    for path in &gguf_files {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(caps) = split_re.captures(stem) {
            let base = caps[1].to_string();
            let part: u32 = caps[2].parse().unwrap_or(0);
            let total: u32 = caps[3].parse().unwrap_or(0);
            split_groups.entry(base).or_default().push((path.clone(), part, total));
        } else {
            standalone.push(path.clone());
        }
    }

    // Process standalone (non-split) models
    for path in &standalone {
        if let Some(model) = parse_gguf_model(path, None) {
            tracing::debug!("  Found: {} ({}, {})", model.name, model.parameters, model.quantization);
            models.push(model);
        }
    }

    // Process split groups: create one ModelInfo per group using shard 00001 as the path
    for (base, mut shards) in split_groups {
        shards.sort_by_key(|s| s.1); // sort by part number
        let total_parts = shards.first().map(|s| s.2).unwrap_or(0);
        let present_parts = shards.len() as u32;
        let total_size: u64 = shards.iter().map(|s| {
            std::fs::metadata(&s.0).map(|m| m.len()).unwrap_or(0)
        }).sum();

        // Use the first shard (00001) as the model path for loading
        let first_shard = shards.iter().find(|s| s.1 == 1).map(|s| &s.0);
        let model_path = first_shard.unwrap_or(&shards[0].0).clone();

        let split_info = SplitInfo {
            total_parts,
            present_parts,
            complete: present_parts == total_parts,
            total_size_bytes: total_size,
        };

        if let Some(mut model) = parse_gguf_model(&model_path, Some(&split_info)) {
            model.name = base.clone();
            model.size_bytes = total_size;
            model.split_info = Some(split_info);
            if first_shard.is_some() {
                model.path = model_path;
            }
            tracing::debug!(
                "  Found split: {} ({}/{} parts, {}, {})",
                model.name, present_parts, total_parts, model.parameters, model.quantization
            );
            models.push(model);
        }
    }

    Ok(())
}

/// Recursively collect all .gguf file paths
fn collect_gguf_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_gguf_files(&path, files)?;
        } else if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "gguf" {
                files.push(path);
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
fn parse_gguf_model(path: &Path, split_info: Option<&SplitInfo>) -> Option<ModelInfo> {
    let filename = path.file_stem()?.to_str()?;
    let metadata = std::fs::metadata(path).ok()?;

    // For split files, strip the -NNNNN-of-NNNNN suffix for name extraction
    let split_re = Regex::new(r"-\d{5}-of-\d{5}$").unwrap();
    let clean_name = split_re.replace(filename, "").to_string();

    let name = clean_name.clone();
    let lower = clean_name.to_lowercase();

    // Extract quantization
    let quantization = extract_quantization(&lower);

    // Extract parameter count from filename first
    let mut parameters = extract_parameters(&lower);

    // If still unknown, estimate from file size
    if parameters == "unknown" {
        let total_bytes = split_info.map(|s| s.total_size_bytes).unwrap_or(metadata.len());
        parameters = estimate_parameters_from_size(total_bytes, &quantization);
    }

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
        split_info: None,
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

/// Estimate parameter count from file size and quantization
fn estimate_parameters_from_size(size_bytes: u64, quantization: &str) -> String {
    let size_gb = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    // Approximate bytes per parameter for common quantization types
    let bytes_per_param: f64 = match quantization.to_uppercase().as_str() {
        "F32" => 4.0,
        "F16" | "BF16" => 2.0,
        "Q8_0" => 1.1,
        "Q6_K" => 0.85,
        "Q5_K_M" | "Q5_K_S" | "Q5_1" | "Q5_0" => 0.7,
        "Q4_K_M" | "Q4_K_S" | "Q4_1" | "Q4_0" => 0.6,
        "Q3_K_M" | "Q3_K_L" | "Q3_K_S" => 0.5,
        "Q2_K" => 0.4,
        q if q.starts_with("IQ") => 0.45,
        _ => 0.65, // reasonable middle ground
    };

    let estimated_params = size_gb / bytes_per_param;

    // Map to nearest standard size
    let sizes: &[(f64, &str)] = &[
        (0.3, "0.5B"), (0.7, "0.5B"),
        (1.2, "1B"), (1.7, "1.5B"),
        (2.5, "3B"), (5.0, "7B"),
        (7.0, "8B"), (10.0, "13B"),
        (12.0, "14B"), (18.0, "22B"),
        (28.0, "32B"), (32.0, "34B"),
        (60.0, "70B"), (68.0, "72B"),
        (100.0, "123B"), (300.0, "405B"),
    ];

    let mut best = "unknown";
    let mut best_dist = f64::MAX;
    for &(threshold, label) in sizes {
        let dist = (estimated_params - threshold).abs();
        if dist < best_dist {
            best_dist = dist;
            best = label;
        }
    }

    // Only return estimate if it's reasonably close (within 3x)
    if best != "unknown" && best_dist < estimated_params * 2.0 {
        format!("~{}", best)
    } else if estimated_params >= 0.3 {
        // Return a rough B count
        if estimated_params >= 100.0 {
            format!("~{}B", estimated_params.round() as u64)
        } else if estimated_params >= 1.0 {
            format!("~{:.0}B", estimated_params)
        } else {
            format!("~{:.1}B", estimated_params)
        }
    } else {
        "unknown".to_string()
    }
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
