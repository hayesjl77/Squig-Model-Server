use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::response::sse::Event;
use futures::stream::Stream;
use parking_lot::RwLock;
use tokio::process::{Child, Command};

use crate::api::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::api::completions::{CompletionRequest, CompletionResponse};
use crate::api::embeddings::{EmbeddingRequest, EmbeddingResponse};
use crate::config::InferenceSettings;
use crate::models::ModelInfo;

use super::hardware::detect_hardware;
use super::smart_defaults::compute_smart_settings;
use super::types::{InferenceMetrics, MetricsSnapshot};

/// Manages llama.cpp server processes — one per loaded model.
/// Each model gets its own llama-server sidecar on a unique port.
pub struct InferenceManager {
    /// Map of model_name -> running backend
    backends: RwLock<HashMap<String, Arc<ModelBackend>>>,
    /// Next available port for spawning backends
    next_port: RwLock<u16>,
    /// Aggregate metrics
    metrics: Arc<InferenceMetrics>,
    /// Per-backend paths: "vulkan" -> "/path/to/llama-server", "cuda" -> "...", etc.
    backend_paths: HashMap<String, String>,
}

pub struct ModelBackend {
    pub model_name: String,
    pub port: u16,
    pub process: RwLock<Option<Child>>,
    pub client: reqwest::Client,
    pub base_url: String,
    pub metrics: Arc<InferenceMetrics>,
}

impl InferenceManager {
    /// Kill any orphaned llama-server processes left over from a previous run.
    /// Called once at startup before any new servers are spawned.
    pub fn kill_orphan_llama_servers() {
        use std::process::Command as StdCommand;

        // Use pkill to find and kill any lingering llama-server processes
        let output = StdCommand::new("pgrep")
            .args(["-f", "llama-server"])
            .output();

        match output {
            Ok(out) if !out.stdout.is_empty() => {
                let pids = String::from_utf8_lossy(&out.stdout);
                let my_pid = std::process::id();
                let mut killed = 0;
                for line in pids.lines() {
                    if let Ok(pid) = line.trim().parse::<u32>() {
                        // Don't kill ourselves
                        if pid == my_pid {
                            continue;
                        }
                        // Send SIGKILL
                        let _ = StdCommand::new("kill")
                            .args(["-9", &pid.to_string()])
                            .output();
                        killed += 1;
                    }
                }
                if killed > 0 {
                    tracing::warn!(
                        "Killed {} orphaned llama-server process(es) from a previous run",
                        killed
                    );
                }
            }
            _ => {
                tracing::debug!("No orphaned llama-server processes found");
            }
        }
    }

    pub async fn new(settings: &InferenceSettings) -> Result<Self> {
        // Build the backend_paths map from config + auto-discovery
        let mut backend_paths = settings.backend_paths.clone();

        // Legacy: if llama_server_path is set and backend_paths is empty,
        // use it as the path for the configured gpu_backend
        if backend_paths.is_empty() && !settings.llama_server_path.is_empty() {
            let backend_key = if settings.gpu_backend == "auto" {
                let hw = detect_hardware();
                hw.recommended_backend.clone()
            } else {
                settings.gpu_backend.clone()
            };
            backend_paths.insert(backend_key, settings.llama_server_path.clone());
        }

        // Auto-discover builds if we still have no paths
        if backend_paths.is_empty() {
            backend_paths = discover_llama_servers();
        }

        // If still nothing, try the old single-binary finder as fallback
        if backend_paths.is_empty() {
            if let Ok(path) = find_llama_server() {
                let hw = detect_hardware();
                backend_paths.insert(hw.recommended_backend.clone(), path);
            }
        }

        if backend_paths.is_empty() {
            anyhow::bail!(
                "No llama-server binaries found. Please set [inference.backend_paths] in config.toml \
                 or build llama.cpp with your desired backend(s)."
            );
        }

        for (backend, path) in &backend_paths {
            tracing::info!("Backend '{}' -> {}", backend, path);
        }

        Ok(Self {
            backends: RwLock::new(HashMap::new()),
            next_port: RwLock::new(9100),
            metrics: Arc::new(InferenceMetrics::default()),
            backend_paths,
        })
    }

    /// Get list of available (compiled) backends
    pub fn available_backends(&self) -> Vec<String> {
        self.backend_paths.keys().cloned().collect()
    }

    /// Resolve which llama-server binary to use for the given backend setting
    fn resolve_server_path(&self, gpu_backend: &str) -> Result<String> {
        let backend = if gpu_backend == "auto" {
            let hw = detect_hardware();
            hw.recommended_backend.clone()
        } else {
            gpu_backend.to_string()
        };

        // Direct match
        if let Some(path) = self.backend_paths.get(&backend) {
            return Ok(path.clone());
        }

        // Fallback: if "cpu" requested but not in map, any backend binary works
        // (llama-server always supports CPU even when compiled with GPU)
        if backend == "cpu" {
            if let Some(path) = self.backend_paths.values().next() {
                tracing::info!("No dedicated CPU build; using {} (CPU fallback via -ngl 0)", path);
                return Ok(path.clone());
            }
        }

        // Fallback: use whatever we have and warn
        if let Some((fallback_backend, path)) = self.backend_paths.iter().next() {
            tracing::warn!(
                "Backend '{}' not available, falling back to '{}'",
                backend,
                fallback_backend
            );
            return Ok(path.clone());
        }

        anyhow::bail!(
            "No llama-server binary available for backend '{}'. \
             Available backends: {:?}",
            backend,
            self.backend_paths.keys().collect::<Vec<_>>()
        )
    }

    /// Load a model by spawning a llama-server process.
    /// Settings are automatically tuned via smart defaults to fit the model in available memory.
    pub async fn load_model(&self, model: ModelInfo, settings: &InferenceSettings) -> Result<()> {
        let model_name = model.name.clone();

        // Check if already loaded
        if self.backends.read().contains_key(&model_name) {
            tracing::info!("Model '{}' is already loaded", model_name);
            return Ok(());
        }

        // Compute smart defaults based on model size vs hardware
        let settings = &compute_smart_settings(&model, settings);

        // Allocate port
        let port = {
            let mut p = self.next_port.write();
            let port = *p;
            *p += 1;
            port
        };

        // Build llama-server command - resolve the right binary for the requested backend
        let server_path = self.resolve_server_path(&settings.gpu_backend)?;
        let mut cmd = Command::new(&server_path);
        cmd.arg("-m")
            .arg(&model.path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-ngl")
            .arg(settings.gpu_layers.to_string())
            .arg("-c")
            .arg(settings.context_size.to_string())
            .arg("-np")
            .arg(settings.parallel_slots.to_string());

        // CPU threads
        if settings.threads != -1 {
            cmd.arg("-t").arg(settings.threads.to_string());
        }
        if settings.threads_batch != -1 {
            cmd.arg("-tb").arg(settings.threads_batch.to_string());
        }

        // Batch sizes
        cmd.arg("-b").arg(settings.batch_size.to_string());
        cmd.arg("-ub").arg(settings.ubatch_size.to_string());

        // Flash attention (newer llama.cpp requires -fa on/off/auto)
        if settings.flash_attention {
            cmd.arg("-fa").arg("on");
        } else {
            cmd.arg("-fa").arg("off");
        }

        // Continuous batching
        if !settings.continuous_batching {
            cmd.arg("-nocb");
        }

        // KV cache quantization (separate K and V types)
        if settings.kv_cache_type_k != "f16" {
            cmd.arg("-ctk").arg(&settings.kv_cache_type_k);
        }
        if settings.kv_cache_type_v != "f16" {
            cmd.arg("-ctv").arg(&settings.kv_cache_type_v);
        }

        // Memory locking
        if settings.mlock {
            cmd.arg("--mlock");
        }

        // Memory mapping
        if settings.no_mmap {
            cmd.arg("--no-mmap");
        }

        // Max tokens to predict
        if settings.n_predict != -1 {
            cmd.arg("-n").arg(settings.n_predict.to_string());
        }

        // RoPE settings
        if !settings.rope_scaling.is_empty() && settings.rope_scaling != "none" {
            cmd.arg("--rope-scaling").arg(&settings.rope_scaling);
        }
        if settings.rope_freq_base > 0.0 {
            cmd.arg("--rope-freq-base").arg(settings.rope_freq_base.to_string());
        }
        if settings.rope_freq_scale > 0.0 {
            cmd.arg("--rope-freq-scale").arg(settings.rope_freq_scale.to_string());
        }

        // Multi-GPU split mode
        if settings.split_mode != "layer" {
            cmd.arg("-sm").arg(&settings.split_mode);
        }
        if settings.main_gpu != 0 {
            cmd.arg("-mg").arg(settings.main_gpu.to_string());
        }
        if !settings.tensor_split.is_empty() {
            cmd.arg("-ts").arg(&settings.tensor_split);
        }

        // Prompt caching
        if !settings.cache_prompt {
            cmd.arg("--no-cache-prompt");
        }

        // Warmup
        if !settings.warmup {
            cmd.arg("--no-warmup");
        }

        // Enable Jinja templates for tool calling support
        cmd.arg("--jinja");

        // Enable embedding endpoint (/v1/embeddings)
        cmd.arg("--embedding")
            .arg("--pooling").arg("mean");

        // Speculative decoding
        if settings.speculative.enabled && !settings.speculative.draft_model.is_empty() {
            cmd.arg("-md")
                .arg(&settings.speculative.draft_model)
                .arg("--draft-max")
                .arg(settings.speculative.draft_max.to_string())
                .arg("--draft-min")
                .arg(settings.speculative.draft_min.to_string());
        }

        // GPU backend
        let hw = detect_hardware();
        let backend = if settings.gpu_backend == "auto" {
            &hw.recommended_backend
        } else {
            &settings.gpu_backend
        };
        tracing::info!(
            "Starting llama-server for '{}' on port {} (backend: {}, binary: {})",
            model_name,
            port,
            backend,
            server_path
        );

        // On Linux: tell the kernel to send SIGKILL to this child if our
        // process dies for ANY reason (crash, SIGKILL, abort, etc.).
        // This prevents orphaned llama-server processes.
        #[cfg(unix)]
        {
            // SAFETY: prctl(PR_SET_PDEATHSIG) is async-signal-safe and
            // only affects the about-to-exec child process.
            unsafe {
                cmd.pre_exec(|| {
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                    Ok(())
                });
            }
        }

        // Suppress stdout/stderr or pipe to tracing
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().context(format!(
            "Failed to spawn llama-server at '{}' for backend '{}'",
            server_path, backend
        ))?;

        let base_url = format!("http://127.0.0.1:{}", port);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(600)) // 10m timeout for long generations
            .build()?;

        // Wait for the server to become ready
        let health_url = format!("{}/health", base_url);
        let ready_client = client.clone();
        let max_wait = std::time::Duration::from_secs(120);
        let start = std::time::Instant::now();

        tracing::info!("Waiting for llama-server to become ready...");
        loop {
            if start.elapsed() > max_wait {
                anyhow::bail!(
                    "llama-server for '{}' did not become ready within {}s",
                    model_name,
                    max_wait.as_secs()
                );
            }

            // Check if the process has already exited (crashed)
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process exited — read stderr for the error message
                    let stderr_output = if let Some(stderr) = child.stderr.take() {
                        use tokio::io::AsyncReadExt;
                        let mut buf = String::new();
                        let mut reader = tokio::io::BufReader::new(stderr);
                        let _ = reader.read_to_string(&mut buf).await;
                        buf
                    } else {
                        String::new()
                    };
                    let trimmed = stderr_output.trim();
                    let detail = if trimmed.is_empty() {
                        format!("exit code: {}", status)
                    } else {
                        // Show last 5 lines of stderr
                        let last_lines: Vec<&str> = trimmed.lines().rev().take(5).collect();
                        last_lines.into_iter().rev().collect::<Vec<_>>().join("\n")
                    };
                    anyhow::bail!(
                        "llama-server for '{}' crashed on startup:\n{}",
                        model_name,
                        detail
                    );
                }
                Ok(None) => {} // Still running, good
                Err(e) => {
                    tracing::warn!("Failed to check llama-server process status: {}", e);
                }
            }

            match ready_client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => break,
                _ => tokio::time::sleep(std::time::Duration::from_millis(500)).await,
            }
        }
        tracing::info!("llama-server for '{}' is ready on port {}", model_name, port);

        let backend_instance = Arc::new(ModelBackend {
            model_name: model_name.clone(),
            port,
            process: RwLock::new(Some(child)),
            client,
            base_url,
            metrics: self.metrics.clone(),
        });

        self.backends.write().insert(model_name, backend_instance);
        Ok(())
    }

    /// Unload a model by killing its llama-server process
    pub async fn unload_model(&self, model_name: &str) -> Result<()> {
        let backend = self
            .backends
            .write()
            .remove(model_name)
            .context(format!("Model '{}' is not loaded", model_name))?;

        // Take the child process out of the lock before awaiting
        let child = backend.process.write().take();
        if let Some(mut child) = child {
            let _ = child.kill().await;
            tracing::info!("Unloaded model '{}'", model_name);
        }
        Ok(())
    }

    /// Shut down all loaded models — kills every llama-server process.
    /// Called when the application is exiting.
    pub async fn shutdown_all(&self) {
        let backends: Vec<Arc<ModelBackend>> = {
            let mut map = self.backends.write();
            map.drain().map(|(_, v)| v).collect()
        };
        for backend in backends {
            let child = backend.process.write().take();
            if let Some(mut child) = child {
                let _ = child.kill().await;
                tracing::info!("Shutdown: killed llama-server for '{}'", backend.model_name);
            }
        }
    }

    /// Get the backend for a specific model
    pub async fn get_backend(&self, model_name: &str) -> Option<Arc<ModelBackend>> {
        self.backends.read().get(model_name).cloned()
    }

    /// Get any available backend (first loaded model). Useful for embeddings
    /// where the specific model doesn't matter as much as availability.
    pub async fn get_any_backend(&self) -> Option<Arc<ModelBackend>> {
        self.backends.read().values().next().cloned()
    }

    /// List names of currently loaded models
    pub async fn loaded_models(&self) -> Vec<String> {
        self.backends.read().keys().cloned().collect()
    }

    /// Get detailed info about loaded models
    pub async fn loaded_model_details(&self) -> Vec<serde_json::Value> {
        self.backends
            .read()
            .values()
            .map(|b| {
                serde_json::json!({
                    "name": b.model_name,
                    "port": b.port,
                    "base_url": b.base_url,
                    "status": "running",
                })
            })
            .collect()
    }

    /// Get aggregate metrics
    pub async fn get_metrics(&self) -> MetricsSnapshot {
        self.metrics.snapshot()
    }
}

impl ModelBackend {
    /// Forward a chat completion request to the llama.cpp server
    pub async fn chat_completions(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        // Build forwarded request (non-streaming)
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "top_p": request.top_p.unwrap_or(0.95),
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "stream": false,
            "stop": request.stop,
            "seed": request.seed,
        });

        // Forward optional sampling parameters
        let obj = body.as_object_mut().unwrap();
        if let Some(v) = request.presence_penalty { obj.insert("presence_penalty".into(), v.into()); }
        if let Some(v) = request.frequency_penalty { obj.insert("frequency_penalty".into(), v.into()); }
        if let Some(v) = request.top_k { obj.insert("top_k".into(), v.into()); }
        if let Some(v) = request.min_p { obj.insert("min_p".into(), v.into()); }
        if let Some(v) = request.repeat_penalty { obj.insert("repeat_penalty".into(), v.into()); }
        if let Some(v) = request.repeat_last_n { obj.insert("repeat_last_n".into(), v.into()); }
        if let Some(v) = request.typical_p { obj.insert("typical_p".into(), v.into()); }
        if let Some(v) = request.mirostat { obj.insert("mirostat".into(), v.into()); }
        if let Some(v) = request.mirostat_tau { obj.insert("mirostat_tau".into(), v.into()); }
        if let Some(v) = request.mirostat_eta { obj.insert("mirostat_eta".into(), v.into()); }
        if let Some(v) = &request.grammar { obj.insert("grammar".into(), v.clone().into()); }
        if let Some(v) = &request.response_format { obj.insert("response_format".into(), v.clone()); }
        if let Some(v) = request.dynatemp_range { obj.insert("dynatemp_range".into(), v.into()); }
        if let Some(v) = request.dynatemp_exponent { obj.insert("dynatemp_exponent".into(), v.into()); }
        // Tool calling
        if let Some(ref tools) = request.tools { obj.insert("tools".into(), serde_json::to_value(tools).unwrap()); }
        if let Some(ref tc) = request.tool_choice { obj.insert("tool_choice".into(), tc.clone()); }
        if let Some(v) = request.parallel_tool_calls { obj.insert("parallel_tool_calls".into(), v.into()); }

        let start = std::time::Instant::now();
        self.metrics
            .active_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to reach llama-server")?;

        let elapsed = start.elapsed();
        self.metrics
            .active_requests
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics
            .total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics
            .total_inference_ms
            .fetch_add(elapsed.as_millis() as u64, std::sync::atomic::Ordering::Relaxed);

        let response: ChatCompletionResponse = resp
            .json()
            .await
            .context("Failed to parse llama-server response")?;

        self.metrics.total_tokens_generated.fetch_add(
            response.usage.completion_tokens as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        self.metrics.total_prompt_tokens.fetch_add(
            response.usage.prompt_tokens as u64,
            std::sync::atomic::Ordering::Relaxed,
        );

        Ok(response)
    }

    /// Forward a streaming chat completion request
    pub async fn chat_completions_stream(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "top_p": request.top_p.unwrap_or(0.95),
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "stream": true,
            "stop": request.stop,
            "seed": request.seed,
        });

        // Forward optional sampling parameters
        let obj = body.as_object_mut().unwrap();
        if let Some(v) = request.presence_penalty { obj.insert("presence_penalty".into(), v.into()); }
        if let Some(v) = request.frequency_penalty { obj.insert("frequency_penalty".into(), v.into()); }
        if let Some(v) = request.top_k { obj.insert("top_k".into(), v.into()); }
        if let Some(v) = request.min_p { obj.insert("min_p".into(), v.into()); }
        if let Some(v) = request.repeat_penalty { obj.insert("repeat_penalty".into(), v.into()); }
        if let Some(v) = request.repeat_last_n { obj.insert("repeat_last_n".into(), v.into()); }
        if let Some(v) = request.typical_p { obj.insert("typical_p".into(), v.into()); }
        if let Some(v) = request.mirostat { obj.insert("mirostat".into(), v.into()); }
        if let Some(v) = request.mirostat_tau { obj.insert("mirostat_tau".into(), v.into()); }
        if let Some(v) = request.mirostat_eta { obj.insert("mirostat_eta".into(), v.into()); }
        if let Some(v) = &request.grammar { obj.insert("grammar".into(), v.clone().into()); }
        if let Some(v) = &request.response_format { obj.insert("response_format".into(), v.clone()); }
        if let Some(v) = request.dynatemp_range { obj.insert("dynatemp_range".into(), v.into()); }
        if let Some(v) = request.dynatemp_exponent { obj.insert("dynatemp_exponent".into(), v.into()); }
        // Tool calling
        if let Some(ref tools) = request.tools { obj.insert("tools".into(), serde_json::to_value(tools).unwrap()); }
        if let Some(ref tc) = request.tool_choice { obj.insert("tool_choice".into(), tc.clone()); }
        if let Some(v) = request.parallel_tool_calls { obj.insert("parallel_tool_calls".into(), v.into()); }

        self.metrics
            .active_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to reach llama-server")?;

        let metrics = self.metrics.clone();
        let byte_stream = resp.bytes_stream();

        // Convert the byte stream from llama.cpp into SSE events
        let stream = async_stream::stream! {
            use futures::StreamExt;
            let mut buffer = String::new();

            tokio::pin!(byte_stream);
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete SSE lines
                        while let Some(pos) = buffer.find("\n\n") {
                            let message = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            for line in message.lines() {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    if data.trim() == "[DONE]" {
                                        metrics.active_requests.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                                        metrics.total_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        yield Ok(Event::default().data("[DONE]"));
                                    } else {
                                        // Count tokens as they stream
                                        metrics.total_tokens_generated.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        yield Ok(Event::default().data(data.to_string()));
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        };

        Ok(stream)
    }

    /// Forward a text completion request
    pub async fn completions(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/v1/completions", self.base_url);

        let body = serde_json::json!({
            "model": request.model,
            "prompt": request.prompt,
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "temperature": request.temperature.unwrap_or(0.7),
            "top_p": request.top_p.unwrap_or(0.95),
            "stream": false,
            "stop": request.stop,
            "seed": request.seed,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to reach llama-server")?;

        let response: CompletionResponse = resp
            .json()
            .await
            .context("Failed to parse response")?;

        Ok(response)
    }

    /// Forward an embedding request to llama-server's /v1/embeddings endpoint
    pub async fn embeddings(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse> {
        let url = format!("{}/v1/embeddings", self.base_url);

        let input = match &request.input {
            crate::api::embeddings::EmbeddingInput::Single(s) => serde_json::json!(s),
            crate::api::embeddings::EmbeddingInput::Multiple(v) => serde_json::json!(v),
        };

        let body = serde_json::json!({
            "input": input,
            "model": request.model,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to reach llama-server for embeddings")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "llama-server embedding endpoint returned {}: {}",
                status,
                body_text
            );
        }

        let response: EmbeddingResponse = resp
            .json()
            .await
            .context("Failed to parse embedding response")?;

        Ok(response)
    }
}

/// Discover llama-server binaries in known build directories.
/// Looks for build-<backend>/bin/llama-server patterns next to a llama.cpp source dir.
fn discover_llama_servers() -> HashMap<String, String> {
    let mut found = HashMap::new();

    // Build directory patterns: (backend_name, relative_build_dir)
    let patterns = [
        ("vulkan", "llama.cpp/build/bin/llama-server"),
        ("vulkan", "../llama.cpp/build/bin/llama-server"),
        ("cuda", "llama.cpp/build-cuda/bin/llama-server"),
        ("cuda", "../llama.cpp/build-cuda/bin/llama-server"),
        ("rocm", "llama.cpp/build-rocm/bin/llama-server"),
        ("rocm", "../llama.cpp/build-rocm/bin/llama-server"),
        ("cpu", "llama.cpp/build-cpu/bin/llama-server"),
        ("cpu", "../llama.cpp/build-cpu/bin/llama-server"),
    ];

    for (backend, path) in &patterns {
        let p = std::path::Path::new(path);
        if p.exists() {
            // Prefer absolute path
            let abs = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            let abs_str = abs.to_string_lossy().into_owned();
            // Only insert if we haven't already found this backend
            found.entry(backend.to_string()).or_insert_with(|| {
                tracing::info!("Auto-discovered {} backend: {}", backend, abs_str);
                abs_str
            });
        }
    }

    found
}

/// Find a single llama-server binary in common locations (legacy fallback)
fn find_llama_server() -> Result<String> {
    // Check PATH first
    let names = [
        "llama-server",
        "llama-server.exe",
    ];

    for name in &names {
        if which::which(name).is_ok() {
            return Ok(name.to_string());
        }
    }

    // Check common install locations
    let paths = [
        "/usr/local/bin/llama-server",
        "/usr/bin/llama-server",
        "llama.cpp/build/bin/llama-server",
        "../llama.cpp/build/bin/llama-server",
        "C:\\llama.cpp\\build\\bin\\Release\\llama-server.exe",
    ];

    for path in &paths {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    anyhow::bail!(
        "Could not find llama-server binary. Please set [inference.backend_paths] in config.toml \
         or build llama.cpp with your desired backend(s)."
    )
}

impl Drop for ModelBackend {
    fn drop(&mut self) {
        // Kill the llama-server process when backend is dropped
        if let Some(mut child) = self.process.write().take() {
            let _ = child.start_kill();
            tracing::info!("Killed llama-server for '{}'", self.model_name);
        }
    }
}
