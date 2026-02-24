//! Smart default parameter computation for model loading.
//!
//! Given a model's file size and the detected hardware, this module computes
//! sensible inference settings so the model loads successfully without manual tuning.

use crate::config::InferenceSettings;
use crate::models::ModelInfo;
use super::hardware::{detect_hardware, HardwareInfo};

/// Adjust inference settings so the given model will fit in available memory.
///
/// The strategy:
/// 1. Estimate the total memory budget (VRAM for GPU offload, or system RAM for CPU).
/// 2. If the model file + KV cache + overhead won't fit, progressively:
///    a. Reduce context size
///    b. Reduce parallel slots
///    c. Enable aggressive KV cache quantization
///    d. Reduce GPU layers to do a partial offload
/// 3. Always enable flash attention (saves KV memory significantly).
pub fn compute_smart_settings(model: &ModelInfo, base: &InferenceSettings) -> InferenceSettings {
    let hw = detect_hardware();
    compute_smart_settings_with_hw(model, base, &hw)
}

pub fn compute_smart_settings_with_hw(
    model: &ModelInfo,
    base: &InferenceSettings,
    hw: &HardwareInfo,
) -> InferenceSettings {
    let mut s = base.clone();

    let model_size_mb = model.size_bytes as f64 / (1024.0 * 1024.0);

    // Determine available memory budget in MB
    let vram_mb: f64 = hw.gpus.iter()
        .filter_map(|g| g.vram_mb)
        .map(|v| v as f64)
        .sum();

    let system_ram_mb = hw.available_memory_gb * 1024.0;

    // Use VRAM as primary budget if we have a GPU and gpu_layers != 0
    let has_gpu = vram_mb > 0.0 && s.gpu_layers != 0;
    let primary_budget_mb = if has_gpu { vram_mb } else { system_ram_mb };

    // Reserve overhead: OS/driver ~500MB for GPU, ~2GB for system RAM
    let overhead_mb = if has_gpu { 500.0 } else { 2048.0 };
    let usable_mb = (primary_budget_mb - overhead_mb).max(512.0);

    tracing::info!(
        "Smart defaults for '{}': model={:.0}MB, budget={:.0}MB ({}), usable={:.0}MB",
        model.name,
        model_size_mb,
        primary_budget_mb,
        if has_gpu { "VRAM" } else { "RAM" },
        usable_mb,
    );

    // Always enable flash attention — it's strictly better for memory
    s.flash_attention = true;

    // Estimate KV cache memory per token (in bytes) at the given context size
    // Rough formula: per slot, KV cache ≈ 2 * n_layers * d_model * ctx * dtype_size
    // We use a simplified heuristic based on model size class
    let kv_bytes_per_token_per_slot = estimate_kv_bytes_per_token(model_size_mb, &s.kv_cache_type_k);

    // Calculate how much memory is left after the model weights
    let remaining_mb = usable_mb - model_size_mb;

    if remaining_mb < 256.0 {
        // Model barely fits — aggressive mode
        tracing::warn!(
            "Model '{}' barely fits ({:.0}MB remaining). Applying aggressive memory savings.",
            model.name, remaining_mb
        );

        // Minimum viable settings
        s.context_size = 2048;
        s.parallel_slots = 1;
        s.kv_cache_type_k = "q4_0".to_string();
        s.kv_cache_type_v = "q4_0".to_string();
        s.continuous_batching = true;
        s.batch_size = 512;
        s.ubatch_size = 256;

        // If model doesn't fit at ALL in VRAM, do partial offload
        if has_gpu && model_size_mb > usable_mb {
            // Estimate layers that fit: rough ratio of usable VRAM to model size
            let ratio = usable_mb / model_size_mb;
            let estimated_layers = (ratio * 80.0).floor() as i32; // assume ~80 layers max
            s.gpu_layers = estimated_layers.max(0);
            tracing::info!(
                "Partial GPU offload: {} layers (model too large for full offload)",
                s.gpu_layers
            );
        }
    } else if remaining_mb < 1024.0 {
        // Tight fit — reduce context and slots
        tracing::info!(
            "Tight memory for '{}' ({:.0}MB remaining). Reducing context & slots.",
            model.name, remaining_mb
        );

        s.context_size = pick_context_size(remaining_mb, kv_bytes_per_token_per_slot, 1);
        s.parallel_slots = 1;
        // Use quantized KV cache
        if s.kv_cache_type_k == "f16" || s.kv_cache_type_k == "f32" {
            s.kv_cache_type_k = "q8_0".to_string();
        }
        if s.kv_cache_type_v == "f16" || s.kv_cache_type_v == "f32" {
            s.kv_cache_type_v = "q8_0".to_string();
        }
        s.batch_size = 1024;
        s.ubatch_size = 512;
    } else if remaining_mb < 4096.0 {
        // Moderate — keep defaults but cap context if it would exceed budget
        let slots = base.parallel_slots.min(2);
        let max_ctx = pick_context_size(remaining_mb, kv_bytes_per_token_per_slot, slots);
        s.context_size = s.context_size.min(max_ctx);
        s.parallel_slots = slots;

        // Ensure KV quant is at least q8_0
        if s.kv_cache_type_k == "f16" || s.kv_cache_type_k == "f32" {
            s.kv_cache_type_k = "q8_0".to_string();
        }
        if s.kv_cache_type_v == "f16" || s.kv_cache_type_v == "f32" {
            s.kv_cache_type_v = "q8_0".to_string();
        }
    } else {
        // Plenty of room — validate the base settings still fit
        let slots = base.parallel_slots;
        let max_ctx = pick_context_size(remaining_mb, kv_bytes_per_token_per_slot, slots);
        s.context_size = s.context_size.min(max_ctx);
        // Keep user's other settings as-is
    }

    // Ensure gpu_layers = -1 (all) if model fits fully, unless we already set partial offload
    if has_gpu && model_size_mb < usable_mb && s.gpu_layers != 0 {
        s.gpu_layers = -1; // Offload everything
    }

    // Ensure context_size has a sane minimum
    s.context_size = s.context_size.max(512);

    tracing::info!(
        "Smart defaults result: ctx={}, slots={}, gpu_layers={}, kv_k={}, kv_v={}, flash={}",
        s.context_size,
        s.parallel_slots,
        s.gpu_layers,
        s.kv_cache_type_k,
        s.kv_cache_type_v,
        s.flash_attention,
    );

    s
}

/// Estimate KV cache bytes per token based on model size class.
/// Larger models have more layers and wider hidden dims → more KV per token.
fn estimate_kv_bytes_per_token(model_size_mb: f64, kv_type: &str) -> f64 {
    // Base estimate for f16 KV cache (bytes per token per slot per layer pair)
    // These are rough but usable heuristics:
    //   ~1B model: ~32 layers, d=2048  → ~128 bytes/token in f16
    //   ~3B model: ~36 layers, d=3072  → ~256 bytes/token
    //   ~7B model: ~32 layers, d=4096  → ~512 bytes/token
    //  ~13B model: ~40 layers, d=5120  → ~800 bytes/token
    //  ~30B model: ~60 layers, d=6656  → ~1600 bytes/token
    //  ~70B model: ~80 layers, d=8192  → ~2600 bytes/token
    // ~120B model: ~96 layers, d=12288 → ~4700 bytes/token
    let base_f16 = if model_size_mb < 1500.0 {
        128.0   // ~1B
    } else if model_size_mb < 3000.0 {
        256.0   // ~3B
    } else if model_size_mb < 6000.0 {
        512.0   // ~7B
    } else if model_size_mb < 10000.0 {
        800.0   // ~13B
    } else if model_size_mb < 22000.0 {
        1600.0  // ~30B
    } else if model_size_mb < 50000.0 {
        2600.0  // ~70B
    } else {
        4700.0  // ~120B+
    };

    // Scale by KV quantization type
    let quant_factor = match kv_type {
        "f32" => 2.0,
        "f16" | "bf16" => 1.0,
        "q8_0" => 0.5,
        "q5_1" => 0.34,
        "q5_0" => 0.31,
        "q4_1" | "iq4_nl" => 0.28,
        "q4_0" => 0.25,
        _ => 0.5, // default to q8_0-ish
    };

    base_f16 * quant_factor
}

/// Pick the largest context size that fits in the remaining memory budget.
/// Steps down from 131072 → 65536 → 32768 → 16384 → 8192 → 4096 → 2048 → 512
fn pick_context_size(remaining_mb: f64, bytes_per_token_per_slot: f64, slots: usize) -> usize {
    let candidates = [131072, 65536, 32768, 16384, 8192, 4096, 2048, 512];

    let remaining_bytes = remaining_mb * 1024.0 * 1024.0;
    // Reserve 20% of remaining for misc overhead (activations, scratch buffers, etc.)
    let kv_budget_bytes = remaining_bytes * 0.8;

    for &ctx in &candidates {
        let kv_usage = bytes_per_token_per_slot * (ctx as f64) * (slots as f64);
        if kv_usage < kv_budget_bytes {
            return ctx;
        }
    }

    512 // absolute minimum
}
