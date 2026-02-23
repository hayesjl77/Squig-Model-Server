use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::response::sse::Event;
use futures::stream::Stream;
use parking_lot::RwLock;
use tokio::process::{Child, Command};

use crate::api::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::api::completions::{CompletionRequest, CompletionResponse};
use crate::config::InferenceSettings;
use crate::models::ModelInfo;

use super::hardware::detect_hardware;
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
    /// Path to llama-server binary
    llama_server_path: String,
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
    pub async fn new(settings: &InferenceSettings) -> Result<Self> {
        // Find llama-server binary
        let llama_server_path = if !settings.llama_server_path.is_empty() {
            settings.llama_server_path.clone()
        } else {
            find_llama_server()?
        };

        tracing::info!("Using llama-server at: {}", llama_server_path);

        Ok(Self {
            backends: RwLock::new(HashMap::new()),
            next_port: RwLock::new(9100), // Backends start at port 9100
            metrics: Arc::new(InferenceMetrics::default()),
            llama_server_path,
        })
    }

    /// Load a model by spawning a llama-server process
    pub async fn load_model(&self, model: ModelInfo, settings: &InferenceSettings) -> Result<()> {
        let model_name = model.name.clone();

        // Check if already loaded
        if self.backends.read().contains_key(&model_name) {
            tracing::info!("Model '{}' is already loaded", model_name);
            return Ok(());
        }

        // Allocate port
        let port = {
            let mut p = self.next_port.write();
            let port = *p;
            *p += 1;
            port
        };

        // Build llama-server command
        let mut cmd = Command::new(&self.llama_server_path);
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

        // Flash attention (newer llama.cpp requires -fa on/off/auto)
        if settings.flash_attention {
            cmd.arg("-fa").arg("on");
        }

        // KV cache quantization
        if settings.kv_cache_type != "f16" {
            cmd.arg("-ctk").arg(&settings.kv_cache_type);
            cmd.arg("-ctv").arg(&settings.kv_cache_type);
        }

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
            "Starting llama-server for '{}' on port {} (backend: {})",
            model_name,
            port,
            backend
        );

        // Suppress stdout/stderr or pipe to tracing
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().context(format!(
            "Failed to spawn llama-server. Is '{}' in PATH?",
            self.llama_server_path
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

    /// Get the backend for a specific model
    pub async fn get_backend(&self, model_name: &str) -> Option<Arc<ModelBackend>> {
        self.backends.read().get(model_name).cloned()
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
        let body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "top_p": request.top_p.unwrap_or(0.95),
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "stream": false,
            "stop": request.stop,
            "seed": request.seed,
        });

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

        let body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "top_p": request.top_p.unwrap_or(0.95),
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "stream": true,
            "stop": request.stop,
            "seed": request.seed,
        });

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
}

/// Find llama-server binary in common locations
fn find_llama_server() -> Result<String> {
    // Check PATH first
    let names = [
        "llama-server",
        "llama-server.exe",
        "llama.cpp/build/bin/llama-server",
    ];

    for name in &names {
        if which::which(name).is_ok() {
            return Ok(name.to_string());
        }
    }

    // Check common install locations
    let paths = [
        // Linux
        "/usr/local/bin/llama-server",
        "/usr/bin/llama-server",
        // User-built
        "llama.cpp/build/bin/llama-server",
        "../llama.cpp/build/bin/llama-server",
        // Windows
        "C:\\llama.cpp\\build\\bin\\Release\\llama-server.exe",
    ];

    for path in &paths {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    anyhow::bail!(
        "Could not find llama-server binary. Please either:\n\
         1. Add it to your PATH\n\
         2. Set 'llama_server_path' in config.toml\n\
         3. Build llama.cpp: git clone https://github.com/ggml-org/llama.cpp && \
            cd llama.cpp && cmake -B build -DGGML_VULKAN=ON && cmake --build build -j$(nproc)"
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
