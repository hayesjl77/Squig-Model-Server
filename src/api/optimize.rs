use axum::{extract::State, Json};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::InferenceSettings;
use crate::inference::detect_hardware;
use crate::server::AppState;

/// Live inference settings that can be updated at runtime
pub struct LiveInferenceSettings(pub RwLock<InferenceSettings>);

impl LiveInferenceSettings {
    pub fn new(settings: InferenceSettings) -> Self {
        Self(RwLock::new(settings))
    }

    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, InferenceSettings> {
        self.0.read()
    }

    pub fn update(&self, f: impl FnOnce(&mut InferenceSettings)) {
        let mut guard = self.0.write();
        f(&mut guard);
    }

    pub fn replace(&self, settings: InferenceSettings) {
        *self.0.write() = settings;
    }
}

// ─── Optimize Endpoint: Ask the model to tune itself ─────────────────────────

/// POST /api/dev/optimize - Ask the loaded model to analyze its own performance
/// and suggest optimal settings
pub async fn self_optimize(State(state): State<AppState>) -> Json<serde_json::Value> {
    // 1. Gather all context
    let hw = detect_hardware();
    let live_settings = state.live_inference.read().clone();
    let perf = state.request_logger.analyze_performance(&live_settings, &hw);

    // 2. Find a loaded model to query
    let loaded = state.inference_manager.loaded_models().await;
    let model_name = match loaded.first() {
        Some(name) => name.clone(),
        None => {
            return Json(serde_json::json!({
                "error": "No model is currently loaded. Load a model first, run some requests, then try optimizing."
            }));
        }
    };

    if perf.total_requests_analyzed < 3 {
        return Json(serde_json::json!({
            "error": "Need at least 3 inference requests for meaningful analysis. Send more chat messages first."
        }));
    }

    // 3. Build the analysis prompt
    let gpu_info = hw
        .gpus
        .iter()
        .map(|g| {
            format!(
                "{} ({}MB VRAM)",
                g.name,
                g.vram_mb.unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    let system_prompt = format!(
        r#"You are an expert AI inference optimization engine. You are analyzing your OWN performance metrics and configuration to suggest optimal settings for the llama.cpp inference server you are running on.

CURRENT HARDWARE:
- CPU: {} ({} cores, {} threads)
- RAM: {:.1} GB total, {:.1} GB available
- GPUs: {}
- Has CUDA: {}, Has ROCm: {}, Has Vulkan: {}

CURRENT INFERENCE SETTINGS:
- gpu_layers: {} (-1 means all layers offloaded to GPU)
- context_size: {} tokens
- parallel_slots: {}
- flash_attention: {}
- continuous_batching: {}
- kv_cache_type_k: "{}" (KV cache key quantization)
- kv_cache_type_v: "{}" (KV cache value quantization)
- gpu_backend: "{}"
- speculative_decoding: {}

PERFORMANCE METRICS (from {} inference requests):
- Overall Rating: {}
- Average tokens/sec: {:.1}
- P50 tokens/sec: {:.1}
- P95 tokens/sec: {:.1}
- Average time-to-first-token: {:.0}ms
- Trend: {}
- Detected Bottleneck: {}

RULES:
- Only suggest changes that would MEANINGFULLY improve performance
- Consider the hardware constraints carefully (available RAM, VRAM)
- If settings are already optimal, return an empty changes array
- Be conservative — bad settings can cause crashes or OOM
- context_size: valid range 512-131072 (powers of 2 preferred)
- parallel_slots: valid range 1-16
- gpu_layers: -1 (all) or 0-999 specific count
- kv_cache_type_k / kv_cache_type_v: "f16", "q8_0", or "q4_0"
- gpu_backend: "auto", "cuda", "rocm", "vulkan", "cpu"
- flash_attention: true or false

Respond with ONLY a valid JSON object (no markdown fences, no text outside the JSON):
{{
  "analysis": "Your detailed analysis of current performance and bottlenecks...",
  "changes": [
    {{
      "setting": "setting_name",
      "current_value": "current",
      "recommended_value": "recommended",
      "reason": "Why this change will help",
      "impact": "high|medium|low"
    }}
  ],
  "expected_improvement": "Description of expected improvement after applying all changes",
  "confidence": "high|medium|low",
  "warnings": ["Any warnings about the changes"]
}}"#,
        hw.cpu_name,
        hw.cpu_cores,
        hw.cpu_threads,
        hw.total_memory_gb,
        hw.available_memory_gb,
        if gpu_info.is_empty() { "None detected".to_string() } else { gpu_info },
        hw.has_cuda,
        hw.has_rocm,
        hw.has_vulkan,
        live_settings.gpu_layers,
        live_settings.context_size,
        live_settings.parallel_slots,
        live_settings.flash_attention,
        live_settings.continuous_batching,
        live_settings.kv_cache_type_k,
        live_settings.kv_cache_type_v,
        live_settings.gpu_backend,
        live_settings.speculative.enabled,
        perf.total_requests_analyzed,
        perf.overall_rating,
        perf.avg_tokens_per_second,
        perf.p50_tokens_per_second,
        perf.p95_tokens_per_second,
        perf.avg_time_to_first_token_ms,
        perf.recent_trend,
        perf.bottleneck,
    );

    // 4. Query the loaded model
    let backend = match state.inference_manager.get_backend(&model_name).await {
        Some(b) => b,
        None => {
            return Json(serde_json::json!({
                "error": format!("Model '{}' backend not available", model_name)
            }));
        }
    };

    let chat_request = crate::api::chat::ChatCompletionRequest {
        model: model_name.clone(),
        messages: vec![
            crate::api::chat::ChatMessage {
                role: "system".to_string(),
                content: Some(system_prompt),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            crate::api::chat::ChatMessage {
                role: "user".to_string(),
                content: Some("Analyze the performance metrics and current settings above. Suggest optimal configuration changes. Respond with JSON only.".to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ],
        temperature: Some(0.3), // Low temperature for consistent analytical output
        top_p: Some(0.9),
        max_tokens: Some(2048),
        stream: Some(false),
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        seed: Some(42),
        top_k: None,
        min_p: None,
        repeat_penalty: None,
        repeat_last_n: None,
        typical_p: None,
        mirostat: None,
        mirostat_tau: None,
        mirostat_eta: None,
        grammar: None,
        response_format: None,
        dynatemp_range: None,
        dynatemp_exponent: None,
        tools: None,
        tool_choice: None,
        parallel_tool_calls: None,
    };

    match backend.chat_completions(&chat_request).await {
        Ok(response) => {
            let raw_content = response
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .unwrap_or_default();

            // Try to parse the model's JSON response
            let parsed = parse_optimize_response(&raw_content);

            Json(serde_json::json!({
                "status": "ok",
                "model_used": model_name,
                "raw_response": raw_content,
                "parsed": parsed,
                "current_settings": {
                    "gpu_layers": live_settings.gpu_layers,
                    "context_size": live_settings.context_size,
                    "parallel_slots": live_settings.parallel_slots,
                    "flash_attention": live_settings.flash_attention,
                    "continuous_batching": live_settings.continuous_batching,
                    "kv_cache_type_k": live_settings.kv_cache_type_k,
                    "kv_cache_type_v": live_settings.kv_cache_type_v,
                    "gpu_backend": live_settings.gpu_backend,
                    "threads": live_settings.threads,
                    "batch_size": live_settings.batch_size,
                    "mlock": live_settings.mlock,
                    "cache_prompt": live_settings.cache_prompt,
                    "speculative_enabled": live_settings.speculative.enabled,
                },
                "performance_summary": {
                    "rating": perf.overall_rating,
                    "avg_tps": perf.avg_tokens_per_second,
                    "trend": perf.recent_trend,
                    "bottleneck": perf.bottleneck,
                    "total_requests": perf.total_requests_analyzed,
                },
            }))
        }
        Err(e) => Json(serde_json::json!({
            "error": format!("Failed to query model for optimization: {}", e)
        })),
    }
}

/// Try to extract valid JSON from the model's response
fn parse_optimize_response(raw: &str) -> serde_json::Value {
    // Try direct parse
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        return v;
    }

    // Try to find JSON block within markdown fences
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            let json_str = &raw[start..=end];
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                return v;
            }
        }
    }

    // Return error indicator
    serde_json::json!({
        "parse_error": "Could not parse model response as JSON",
        "raw": raw,
    })
}

// ─── Apply Settings Endpoint ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApplySettingsRequest {
    /// Individual settings to change
    pub changes: Vec<SettingChange>,
    /// Model to reload with new settings (if any model is loaded)
    pub reload_model: Option<String>,
    /// Whether to persist changes to config.toml
    pub save_to_disk: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SettingChange {
    pub setting: String,
    pub value: serde_json::Value,
}

/// POST /api/dev/apply-settings - Apply inference setting changes
pub async fn apply_settings(
    State(state): State<AppState>,
    Json(request): Json<ApplySettingsRequest>,
) -> Json<serde_json::Value> {
    let mut applied: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // Apply each change to the live settings
    for change in &request.changes {
        match apply_single_setting(&state.live_inference, &change.setting, &change.value) {
            Ok(desc) => applied.push(desc),
            Err(e) => errors.push(format!("{}: {}", change.setting, e)),
        }
    }

    // Optionally save to config.toml
    if request.save_to_disk.unwrap_or(false) {
        let mut full_config = state.config.clone();
        full_config.inference = state.live_inference.read().clone();
        let config_path = dirs::home_dir()
            .map(|h| h.join(".squig-models").join("config.toml"))
            .unwrap_or_else(|| std::path::PathBuf::from("config.toml"));
        match toml::to_string_pretty(&full_config) {
            Ok(toml_str) => {
                if let Err(e) = std::fs::write(&config_path, &toml_str) {
                    errors.push(format!("Failed to save config: {}", e));
                } else {
                    applied.push(format!("Saved to {}", config_path.display()));
                }
            }
            Err(e) => errors.push(format!("Failed to serialize config: {}", e)),
        }
    }

    // Reload model if requested
    let mut reload_status = None;
    if let Some(model_name) = &request.reload_model {
        let new_settings = state.live_inference.read().clone();

        // Unload
        if let Err(e) = state.inference_manager.unload_model(model_name).await {
            errors.push(format!("Failed to unload model: {}", e));
        } else {
            // Reload with new settings
            if let Some(model_info) = state.model_registry.find_model(model_name) {
                match state
                    .inference_manager
                    .load_model(model_info.clone(), &new_settings)
                    .await
                {
                    Ok(()) => {
                        reload_status = Some("reloaded");
                    }
                    Err(e) => {
                        errors.push(format!("Failed to reload model: {}", e));
                        reload_status = Some("reload_failed");
                    }
                }
            } else {
                errors.push(format!("Model '{}' not found in registry", model_name));
                reload_status = Some("model_not_found");
            }
        }
    }

    Json(serde_json::json!({
        "status": if errors.is_empty() { "ok" } else { "partial" },
        "applied": applied,
        "errors": errors,
        "reload_status": reload_status,
        "new_settings": {
            "gpu_layers": state.live_inference.read().gpu_layers,
            "context_size": state.live_inference.read().context_size,
            "parallel_slots": state.live_inference.read().parallel_slots,
            "flash_attention": state.live_inference.read().flash_attention,
            "continuous_batching": state.live_inference.read().continuous_batching,
            "kv_cache_type_k": state.live_inference.read().kv_cache_type_k,
            "kv_cache_type_v": state.live_inference.read().kv_cache_type_v,
            "gpu_backend": state.live_inference.read().gpu_backend,
            "threads": state.live_inference.read().threads,
            "threads_batch": state.live_inference.read().threads_batch,
            "batch_size": state.live_inference.read().batch_size,
            "ubatch_size": state.live_inference.read().ubatch_size,
            "mlock": state.live_inference.read().mlock,
            "no_mmap": state.live_inference.read().no_mmap,
            "n_predict": state.live_inference.read().n_predict,
            "cache_prompt": state.live_inference.read().cache_prompt,
            "warmup": state.live_inference.read().warmup,
            "speculative_enabled": state.live_inference.read().speculative.enabled,
        },
    }))
}

/// GET /api/dev/settings - Get current live inference settings
pub async fn get_settings(State(state): State<AppState>) -> Json<serde_json::Value> {
    let s = state.live_inference.read().clone();
    Json(serde_json::json!({
        "gpu_layers": s.gpu_layers,
        "context_size": s.context_size,
        "parallel_slots": s.parallel_slots,
        "flash_attention": s.flash_attention,
        "continuous_batching": s.continuous_batching,
        "kv_cache_type_k": s.kv_cache_type_k,
        "kv_cache_type_v": s.kv_cache_type_v,
        "gpu_backend": s.gpu_backend,
        "available_backends": state.inference_manager.available_backends(),
        "threads": s.threads,
        "threads_batch": s.threads_batch,
        "batch_size": s.batch_size,
        "ubatch_size": s.ubatch_size,
        "mlock": s.mlock,
        "no_mmap": s.no_mmap,
        "n_predict": s.n_predict,
        "rope_scaling": s.rope_scaling,
        "rope_freq_base": s.rope_freq_base,
        "rope_freq_scale": s.rope_freq_scale,
        "split_mode": s.split_mode,
        "main_gpu": s.main_gpu,
        "tensor_split": s.tensor_split,
        "cache_prompt": s.cache_prompt,
        "warmup": s.warmup,
        "speculative": {
            "enabled": s.speculative.enabled,
            "draft_model": s.speculative.draft_model,
            "draft_max": s.speculative.draft_max,
            "draft_min": s.speculative.draft_min,
        },
    }))
}

fn apply_single_setting(
    live: &Arc<LiveInferenceSettings>,
    setting: &str,
    value: &serde_json::Value,
) -> Result<String, String> {
    match setting {
        "gpu_layers" => {
            let v = value.as_i64().ok_or("Expected integer")? as i32;
            if v < -1 || v > 9999 {
                return Err("gpu_layers must be -1 to 9999".to_string());
            }
            let old = live.read().gpu_layers;
            live.update(|s| s.gpu_layers = v);
            Ok(format!("gpu_layers: {} → {}", old, v))
        }
        "context_size" => {
            let v = value.as_u64().ok_or("Expected positive integer")? as usize;
            if v < 512 || v > 131072 {
                return Err("context_size must be 512-131072".to_string());
            }
            let old = live.read().context_size;
            live.update(|s| s.context_size = v);
            Ok(format!("context_size: {} → {}", old, v))
        }
        "parallel_slots" => {
            let v = value.as_u64().ok_or("Expected positive integer")? as usize;
            if v < 1 || v > 16 {
                return Err("parallel_slots must be 1-16".to_string());
            }
            let old = live.read().parallel_slots;
            live.update(|s| s.parallel_slots = v);
            Ok(format!("parallel_slots: {} → {}", old, v))
        }
        "flash_attention" => {
            let v = value.as_bool().ok_or("Expected boolean")?;
            let old = live.read().flash_attention;
            live.update(|s| s.flash_attention = v);
            Ok(format!("flash_attention: {} → {}", old, v))
        }
        "continuous_batching" => {
            let v = value.as_bool().ok_or("Expected boolean")?;
            let old = live.read().continuous_batching;
            live.update(|s| s.continuous_batching = v);
            Ok(format!("continuous_batching: {} → {}", old, v))
        }
        "kv_cache_type_k" | "kv_cache_type_v" => {
            let v = value.as_str().ok_or("Expected string")?;
            let valid = ["f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"];
            if !valid.contains(&v) {
                return Err(format!("{} must be one of: {}", setting, valid.join(", ")));
            }
            let v_owned = v.to_string();
            if setting == "kv_cache_type_k" {
                let old = live.read().kv_cache_type_k.clone();
                live.update(|s| s.kv_cache_type_k = v_owned);
                Ok(format!("kv_cache_type_k: {} → {}", old, v))
            } else {
                let old = live.read().kv_cache_type_v.clone();
                live.update(|s| s.kv_cache_type_v = v_owned);
                Ok(format!("kv_cache_type_v: {} → {}", old, v))
            }
        }
        "gpu_backend" => {
            let v = value.as_str().ok_or("Expected string")?;
            if !["auto", "cuda", "rocm", "vulkan", "cpu"].contains(&v) {
                return Err("gpu_backend must be auto, cuda, rocm, vulkan, or cpu".to_string());
            }
            let old = live.read().gpu_backend.clone();
            let v_owned = v.to_string();
            live.update(|s| s.gpu_backend = v_owned);
            Ok(format!("gpu_backend: {} → {}", old, v))
        }
        "threads" => {
            let v = value.as_i64().ok_or("Expected integer")? as i32;
            if v < -1 || v > 256 {
                return Err("threads must be -1 to 256".to_string());
            }
            let old = live.read().threads;
            live.update(|s| s.threads = v);
            Ok(format!("threads: {} → {}", old, v))
        }
        "threads_batch" => {
            let v = value.as_i64().ok_or("Expected integer")? as i32;
            if v < -1 || v > 256 {
                return Err("threads_batch must be -1 to 256".to_string());
            }
            let old = live.read().threads_batch;
            live.update(|s| s.threads_batch = v);
            Ok(format!("threads_batch: {} → {}", old, v))
        }
        "batch_size" => {
            let v = value.as_u64().ok_or("Expected positive integer")? as usize;
            if v < 1 || v > 16384 {
                return Err("batch_size must be 1-16384".to_string());
            }
            let old = live.read().batch_size;
            live.update(|s| s.batch_size = v);
            Ok(format!("batch_size: {} → {}", old, v))
        }
        "ubatch_size" => {
            let v = value.as_u64().ok_or("Expected positive integer")? as usize;
            if v < 1 || v > 8192 {
                return Err("ubatch_size must be 1-8192".to_string());
            }
            let old = live.read().ubatch_size;
            live.update(|s| s.ubatch_size = v);
            Ok(format!("ubatch_size: {} → {}", old, v))
        }
        "mlock" => {
            let v = value.as_bool().ok_or("Expected boolean")?;
            let old = live.read().mlock;
            live.update(|s| s.mlock = v);
            Ok(format!("mlock: {} → {}", old, v))
        }
        "no_mmap" => {
            let v = value.as_bool().ok_or("Expected boolean")?;
            let old = live.read().no_mmap;
            live.update(|s| s.no_mmap = v);
            Ok(format!("no_mmap: {} → {}", old, v))
        }
        "n_predict" => {
            let v = value.as_i64().ok_or("Expected integer")? as i32;
            if v < -1 {
                return Err("n_predict must be >= -1".to_string());
            }
            let old = live.read().n_predict;
            live.update(|s| s.n_predict = v);
            Ok(format!("n_predict: {} → {}", old, v))
        }
        "rope_scaling" => {
            let v = value.as_str().ok_or("Expected string")?;
            if !["", "none", "linear", "yarn"].contains(&v) {
                return Err("rope_scaling must be empty, none, linear, or yarn".to_string());
            }
            let old = live.read().rope_scaling.clone();
            let v_owned = v.to_string();
            live.update(|s| s.rope_scaling = v_owned);
            Ok(format!("rope_scaling: '{}' → '{}'", old, v))
        }
        "rope_freq_base" => {
            let v = value.as_f64().ok_or("Expected number")?;
            let old = live.read().rope_freq_base;
            live.update(|s| s.rope_freq_base = v);
            Ok(format!("rope_freq_base: {} → {}", old, v))
        }
        "rope_freq_scale" => {
            let v = value.as_f64().ok_or("Expected number")?;
            let old = live.read().rope_freq_scale;
            live.update(|s| s.rope_freq_scale = v);
            Ok(format!("rope_freq_scale: {} → {}", old, v))
        }
        "split_mode" => {
            let v = value.as_str().ok_or("Expected string")?;
            if !["none", "layer", "row"].contains(&v) {
                return Err("split_mode must be none, layer, or row".to_string());
            }
            let old = live.read().split_mode.clone();
            let v_owned = v.to_string();
            live.update(|s| s.split_mode = v_owned);
            Ok(format!("split_mode: {} → {}", old, v))
        }
        "main_gpu" => {
            let v = value.as_i64().ok_or("Expected integer")? as i32;
            if v < 0 || v > 15 {
                return Err("main_gpu must be 0-15".to_string());
            }
            let old = live.read().main_gpu;
            live.update(|s| s.main_gpu = v);
            Ok(format!("main_gpu: {} → {}", old, v))
        }
        "tensor_split" => {
            let v = value.as_str().ok_or("Expected string")?;
            let old = live.read().tensor_split.clone();
            let v_owned = v.to_string();
            live.update(|s| s.tensor_split = v_owned);
            Ok(format!("tensor_split: '{}' → '{}'", old, v))
        }
        "cache_prompt" => {
            let v = value.as_bool().ok_or("Expected boolean")?;
            let old = live.read().cache_prompt;
            live.update(|s| s.cache_prompt = v);
            Ok(format!("cache_prompt: {} → {}", old, v))
        }
        "warmup" => {
            let v = value.as_bool().ok_or("Expected boolean")?;
            let old = live.read().warmup;
            live.update(|s| s.warmup = v);
            Ok(format!("warmup: {} → {}", old, v))
        }
        _ => Err(format!("Unknown setting: {}", setting)),
    }
}
