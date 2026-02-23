use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use axum::extract::{Query, State};
use axum::Json;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::inference::detect_hardware;
use crate::server::AppState;

/// Maximum number of log entries to keep in the ring buffer
const MAX_LOG_ENTRIES: usize = 500;

/// A single API request/response log entry
#[derive(Debug, Clone, Serialize)]
pub struct RequestLogEntry {
    pub id: u64,
    pub timestamp: String,
    pub method: String,
    pub path: String,
    pub model: String,
    pub request_summary: String,
    pub response_summary: String,
    pub status_code: u16,
    pub duration_ms: u64,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub tokens_per_second: f64,
    pub time_to_first_token_ms: Option<u64>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
}

/// Per-request performance sample for trend analysis
#[derive(Debug, Clone, Serialize)]
pub struct PerfSample {
    pub timestamp: String,
    pub model: String,
    pub tokens_per_second: f64,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub duration_ms: u64,
    pub time_to_first_token_ms: Option<u64>,
}

/// Ring buffer for API request/response logs
pub struct RequestLogger {
    entries: RwLock<Vec<RequestLogEntry>>,
    perf_samples: RwLock<Vec<PerfSample>>,
    next_id: AtomicU64,
}

impl RequestLogger {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::with_capacity(MAX_LOG_ENTRIES)),
            perf_samples: RwLock::new(Vec::with_capacity(MAX_LOG_ENTRIES)),
            next_id: AtomicU64::new(1),
        }
    }

    /// Record a completed API request
    pub fn log_request(&self, entry: RequestLogEntry) {
        // Also record a perf sample if it was an inference request
        if entry.completion_tokens > 0 {
            let sample = PerfSample {
                timestamp: entry.timestamp.clone(),
                model: entry.model.clone(),
                tokens_per_second: entry.tokens_per_second,
                prompt_tokens: entry.prompt_tokens,
                completion_tokens: entry.completion_tokens,
                duration_ms: entry.duration_ms,
                time_to_first_token_ms: entry.time_to_first_token_ms,
            };
            let mut samples = self.perf_samples.write();
            if samples.len() >= MAX_LOG_ENTRIES {
                samples.remove(0);
            }
            samples.push(sample);
        }

        let mut entries = self.entries.write();
        if entries.len() >= MAX_LOG_ENTRIES {
            entries.remove(0);
        }
        entries.push(entry);
    }

    /// Get next unique ID
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Get all log entries (newest first)
    pub fn get_entries(&self, limit: usize, model_filter: Option<&str>) -> Vec<RequestLogEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| {
                model_filter
                    .map(|f| e.model.to_lowercase().contains(&f.to_lowercase()))
                    .unwrap_or(true)
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get recent performance samples for analysis
    pub fn get_perf_samples(&self, limit: usize) -> Vec<PerfSample> {
        let samples = self.perf_samples.read();
        samples.iter().rev().take(limit).cloned().collect()
    }

    /// Clear all log entries
    pub fn clear(&self) {
        self.entries.write().clear();
        self.perf_samples.write().clear();
    }

    /// Generate performance analysis with suggestions
    pub fn analyze_performance(
        &self,
        config: &crate::config::InferenceSettings,
        hardware: &crate::inference::hardware::HardwareInfo,
    ) -> PerformanceAnalysis {
        let samples = self.perf_samples.read();

        if samples.is_empty() {
            return PerformanceAnalysis {
                overall_rating: "unknown".to_string(),
                avg_tokens_per_second: 0.0,
                p50_tokens_per_second: 0.0,
                p95_tokens_per_second: 0.0,
                avg_time_to_first_token_ms: 0.0,
                avg_prompt_eval_speed: 0.0,
                total_requests_analyzed: 0,
                suggestions: vec![PerformanceSuggestion {
                    severity: "info".to_string(),
                    category: "General".to_string(),
                    title: "No data yet".to_string(),
                    description: "Run some inference requests first, then check back for performance analysis and tuning suggestions.".to_string(),
                    action: None,
                }],
                recent_trend: "stable".to_string(),
                bottleneck: "none".to_string(),
            };
        }

        let count = samples.len();

        // Calculate stats
        let mut tps_values: Vec<f64> = samples.iter().map(|s| s.tokens_per_second).collect();
        tps_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let avg_tps = tps_values.iter().sum::<f64>() / count as f64;
        let p50_tps = percentile(&tps_values, 50);
        let p95_tps = percentile(&tps_values, 95);

        let avg_ttft = samples
            .iter()
            .filter_map(|s| s.time_to_first_token_ms)
            .collect::<Vec<_>>();
        let avg_ttft_ms = if avg_ttft.is_empty() {
            0.0
        } else {
            avg_ttft.iter().sum::<u64>() as f64 / avg_ttft.len() as f64
        };

        // Prompt eval speed (prompt_tokens / duration * 1000 for t/s, but we approximate)
        let prompt_speeds: Vec<f64> = samples
            .iter()
            .filter(|s| s.duration_ms > 0 && s.prompt_tokens > 0)
            .map(|s| s.prompt_tokens as f64 / s.duration_ms as f64 * 1000.0)
            .collect();
        let avg_prompt_speed = if prompt_speeds.is_empty() {
            0.0
        } else {
            prompt_speeds.iter().sum::<f64>() / prompt_speeds.len() as f64
        };

        // Determine trend (compare first half vs second half)
        let recent_trend = if count >= 4 {
            let mid = count / 2;
            let first_half_avg: f64 =
                tps_values[..mid].iter().sum::<f64>() / mid as f64;
            let second_half_avg: f64 =
                tps_values[mid..].iter().sum::<f64>() / (count - mid) as f64;
            if second_half_avg > first_half_avg * 1.1 {
                "improving".to_string()
            } else if second_half_avg < first_half_avg * 0.9 {
                "degrading".to_string()
            } else {
                "stable".to_string()
            }
        } else {
            "insufficient_data".to_string()
        };

        // Generate suggestions
        let mut suggestions = Vec::new();

        // Rate the overall performance
        let overall_rating = if avg_tps >= 30.0 {
            "excellent"
        } else if avg_tps >= 15.0 {
            "good"
        } else if avg_tps >= 5.0 {
            "moderate"
        } else if avg_tps > 0.0 {
            "poor"
        } else {
            "unknown"
        }
        .to_string();

        // Determine bottleneck
        let bottleneck = determine_bottleneck(avg_tps, avg_ttft_ms, config, hardware);

        // --- SUGGESTIONS ---

        // Flash attention
        if !config.flash_attention {
            suggestions.push(PerformanceSuggestion {
                severity: "high".to_string(),
                category: "Memory".to_string(),
                title: "Enable Flash Attention".to_string(),
                description: "Flash attention significantly reduces memory usage and improves throughput, especially for long contexts. It's disabled in your config.".to_string(),
                action: Some("Set flash_attention = true in config.toml".to_string()),
            });
        }

        // KV cache quantization
        if config.kv_cache_type == "f16" {
            suggestions.push(PerformanceSuggestion {
                severity: "medium".to_string(),
                category: "Memory".to_string(),
                title: "Use quantized KV cache".to_string(),
                description: format!(
                    "Your KV cache is using f16. Switching to q8_0 halves KV cache memory with minimal quality loss, or q4_0 for 4x reduction. Current context size: {} tokens.",
                    config.context_size
                ),
                action: Some("Set kv_cache_type = \"q8_0\" in config.toml".to_string()),
            });
        }

        // Context size too large
        if config.context_size > 16384 && avg_tps < 10.0 {
            suggestions.push(PerformanceSuggestion {
                severity: "medium".to_string(),
                category: "Memory".to_string(),
                title: "Reduce context size".to_string(),
                description: format!(
                    "Your context size is {} tokens but inference is slow ({:.1} t/s). Reducing to 8192 or 4096 frees VRAM and speeds up prompt processing.",
                    config.context_size, avg_tps
                ),
                action: Some("Set context_size = 8192 in config.toml".to_string()),
            });
        }

        // GPU layers check
        if config.gpu_layers >= 0 && config.gpu_layers < 99 {
            suggestions.push(PerformanceSuggestion {
                severity: "high".to_string(),
                category: "GPU".to_string(),
                title: "Offload all layers to GPU".to_string(),
                description: format!(
                    "Only {} GPU layers configured. Set to -1 to offload all layers and maximize GPU utilization. Partial offloading forces CPU/GPU data transfer which is a major bottleneck.",
                    config.gpu_layers
                ),
                action: Some("Set gpu_layers = -1 in config.toml".to_string()),
            });
        }

        // Parallel slots vs performance
        if config.parallel_slots > 2 && avg_tps < 8.0 {
            suggestions.push(PerformanceSuggestion {
                severity: "medium".to_string(),
                category: "Throughput".to_string(),
                title: "Reduce parallel slots".to_string(),
                description: format!(
                    "You have {} parallel slots but token speed is low ({:.1} t/s). If you're not handling concurrent requests, reducing to 1-2 frees context memory per slot.",
                    config.parallel_slots, avg_tps
                ),
                action: Some("Set parallel_slots = 1 in config.toml".to_string()),
            });
        }

        // Speculative decoding suggestion
        if !config.speculative.enabled && avg_tps < 20.0 && avg_tps > 3.0 {
            suggestions.push(PerformanceSuggestion {
                severity: "low".to_string(),
                category: "Speed".to_string(),
                title: "Consider speculative decoding".to_string(),
                description: format!(
                    "At {:.1} t/s, speculative decoding with a small draft model could improve throughput by 2-3x for code and structured text generation.",
                    avg_tps
                ),
                action: Some("Set [inference.speculative] enabled = true with a small draft model".to_string()),
            });
        }

        // GPU backend check
        if hardware.has_cuda && config.gpu_backend != "cuda" && config.gpu_backend != "auto" {
            suggestions.push(PerformanceSuggestion {
                severity: "high".to_string(),
                category: "GPU".to_string(),
                title: "Switch to CUDA backend".to_string(),
                description: "NVIDIA GPU detected but not using CUDA backend. CUDA typically provides the best performance for NVIDIA hardware.".to_string(),
                action: Some("Set gpu_backend = \"cuda\" or \"auto\" in config.toml".to_string()),
            });
        }

        if hardware.has_rocm && config.gpu_backend == "vulkan" {
            suggestions.push(PerformanceSuggestion {
                severity: "medium".to_string(),
                category: "GPU".to_string(),
                title: "Consider ROCm backend".to_string(),
                description: "AMD GPU with ROCm detected but using Vulkan. ROCm may offer better performance for supported AMD GPUs.".to_string(),
                action: Some("Set gpu_backend = \"rocm\" in config.toml".to_string()),
            });
        }

        // Slow TTFT
        if avg_ttft_ms > 5000.0 {
            suggestions.push(PerformanceSuggestion {
                severity: "medium".to_string(),
                category: "Latency".to_string(),
                title: "High time-to-first-token".to_string(),
                description: format!(
                    "Average TTFT is {:.0}ms. This suggests slow prompt processing. Consider reducing context size, using quantized KV cache, or ensuring full GPU offloading.",
                    avg_ttft_ms
                ),
                action: None,
            });
        }

        // Degrading performance trend
        if recent_trend == "degrading" {
            suggestions.push(PerformanceSuggestion {
                severity: "medium".to_string(),
                category: "Trend".to_string(),
                title: "Performance is degrading".to_string(),
                description: "Recent requests are slower than earlier ones. This could indicate memory pressure, thermal throttling, or context accumulation. Consider restarting the model or checking system thermals.".to_string(),
                action: Some("Unload and reload the model, or check CPU/GPU temperatures".to_string()),
            });
        }

        // Good performance feedback
        if avg_tps >= 30.0 && suggestions.is_empty() {
            suggestions.push(PerformanceSuggestion {
                severity: "info".to_string(),
                category: "General".to_string(),
                title: "Excellent performance".to_string(),
                description: format!(
                    "At {:.1} tokens/sec your setup is running great. No optimization suggestions at this time.",
                    avg_tps
                ),
                action: None,
            });
        }

        PerformanceAnalysis {
            overall_rating,
            avg_tokens_per_second: (avg_tps * 10.0).round() / 10.0,
            p50_tokens_per_second: (p50_tps * 10.0).round() / 10.0,
            p95_tokens_per_second: (p95_tps * 10.0).round() / 10.0,
            avg_time_to_first_token_ms: (avg_ttft_ms * 10.0).round() / 10.0,
            avg_prompt_eval_speed: (avg_prompt_speed * 10.0).round() / 10.0,
            total_requests_analyzed: count,
            suggestions,
            recent_trend,
            bottleneck,
        }
    }
}

/// Computed timestamp in ISO format
pub fn now_iso() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    chrono::DateTime::from_timestamp(now.as_secs() as i64, now.subsec_nanos())
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceAnalysis {
    pub overall_rating: String,
    pub avg_tokens_per_second: f64,
    pub p50_tokens_per_second: f64,
    pub p95_tokens_per_second: f64,
    pub avg_time_to_first_token_ms: f64,
    pub avg_prompt_eval_speed: f64,
    pub total_requests_analyzed: usize,
    pub suggestions: Vec<PerformanceSuggestion>,
    pub recent_trend: String,
    pub bottleneck: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceSuggestion {
    pub severity: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub action: Option<String>,
}

fn percentile(sorted: &[f64], pct: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (pct as f64 / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn determine_bottleneck(
    avg_tps: f64,
    avg_ttft_ms: f64,
    config: &crate::config::InferenceSettings,
    hardware: &crate::inference::hardware::HardwareInfo,
) -> String {
    // Check memory first
    let total_vram: u64 = hardware
        .gpus
        .iter()
        .filter_map(|g| g.vram_mb)
        .sum();

    if config.gpu_layers >= 0 && config.gpu_layers < 99 {
        return "partial_gpu_offload".to_string();
    }
    if !config.flash_attention && config.context_size > 8192 {
        return "memory_attention".to_string();
    }
    if avg_ttft_ms > 3000.0 {
        return "prompt_processing".to_string();
    }
    if avg_tps < 5.0 && total_vram == 0 {
        return "cpu_bound".to_string();
    }
    if avg_tps < 5.0 {
        return "model_too_large".to_string();
    }
    "none".to_string()
}

// ─── API Route Handlers ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LogsQuery {
    pub limit: Option<usize>,
    pub model: Option<String>,
}

/// GET /api/dev/logs - Get recent API request/response logs
pub async fn api_logs(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> Json<serde_json::Value> {
    let limit = query.limit.unwrap_or(100);
    let entries = state
        .request_logger
        .get_entries(limit, query.model.as_deref());
    Json(serde_json::json!({
        "count": entries.len(),
        "entries": entries,
    }))
}

/// POST /api/dev/logs/clear - Clear all logs
pub async fn clear_logs(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.request_logger.clear();
    Json(serde_json::json!({ "status": "cleared" }))
}

/// GET /api/dev/perf - Get performance analysis with suggestions
pub async fn perf_analysis(State(state): State<AppState>) -> Json<serde_json::Value> {
    let hw = detect_hardware();
    let live_settings = state.live_inference.read().clone();
    let analysis = state
        .request_logger
        .analyze_performance(&live_settings, &hw);
    Json(serde_json::json!(analysis))
}

#[derive(Deserialize)]
pub struct PerfSamplesQuery {
    pub limit: Option<usize>,
}

/// GET /api/dev/perf/samples - Get raw performance samples for charting
pub async fn perf_samples(
    State(state): State<AppState>,
    Query(query): Query<PerfSamplesQuery>,
) -> Json<serde_json::Value> {
    let samples = state
        .request_logger
        .get_perf_samples(query.limit.unwrap_or(100));
    Json(serde_json::json!({
        "count": samples.len(),
        "samples": samples,
    }))
}
