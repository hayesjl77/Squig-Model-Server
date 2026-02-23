use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

/// Metrics tracked per loaded model
#[derive(Debug, Default)]
pub struct InferenceMetrics {
    pub total_requests: AtomicU64,
    pub total_tokens_generated: AtomicU64,
    pub total_prompt_tokens: AtomicU64,
    pub total_inference_ms: AtomicU64,
    pub active_requests: AtomicU64,
}

impl InferenceMetrics {
    pub fn snapshot(&self) -> MetricsSnapshot {
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let total_tokens = self.total_tokens_generated.load(Ordering::Relaxed);
        let total_ms = self.total_inference_ms.load(Ordering::Relaxed);

        MetricsSnapshot {
            total_requests,
            total_tokens_generated: total_tokens,
            total_prompt_tokens: self.total_prompt_tokens.load(Ordering::Relaxed),
            active_requests: self.active_requests.load(Ordering::Relaxed),
            avg_tokens_per_second: if total_ms > 0 {
                (total_tokens as f64 / total_ms as f64) * 1000.0
            } else {
                0.0
            },
            avg_latency_ms: if total_requests > 0 {
                total_ms as f64 / total_requests as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Serialize, Default)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub total_tokens_generated: u64,
    pub total_prompt_tokens: u64,
    pub active_requests: u64,
    pub avg_tokens_per_second: f64,
    pub avg_latency_ms: f64,
}
