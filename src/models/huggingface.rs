use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};
use dashmap::DashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

/// A single GGUF file found inside a HuggingFace repo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfGgufFile {
    /// Filename (e.g. "model-Q4_K_M.gguf")
    pub filename: String,
    /// Size in bytes
    pub size: u64,
    /// Human-readable size
    pub size_human: String,
}

/// A search result representing one HuggingFace model repo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfSearchResult {
    /// Full repo id (e.g. "bartowski/Qwen2.5-Coder-32B-Instruct-GGUF")
    pub repo_id: String,
    /// Last modified timestamp
    pub last_modified: String,
    /// Number of downloads
    pub downloads: u64,
    /// Number of likes
    pub likes: u64,
    /// List of GGUF files available in the repo
    pub gguf_files: Vec<HfGgufFile>,
    /// Tags from HuggingFace
    pub tags: Vec<String>,
}

/// Tracks the progress of a single download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub repo_id: String,
    pub filename: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub progress_pct: f64,
    pub status: DownloadStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Complete,
    Failed,
}

/// Internal state for an active download
struct ActiveDownload {
    repo_id: String,
    filename: String,
    total_bytes: AtomicU64,
    downloaded_bytes: AtomicU64,
    status: parking_lot::RwLock<DownloadStatus>,
    error: parking_lot::RwLock<Option<String>>,
    cancelled: AtomicBool,
}

impl ActiveDownload {
    fn snapshot(&self) -> DownloadProgress {
        let total = self.total_bytes.load(Ordering::Relaxed);
        let downloaded = self.downloaded_bytes.load(Ordering::Relaxed);
        let pct = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        DownloadProgress {
            repo_id: self.repo_id.clone(),
            filename: self.filename.clone(),
            total_bytes: total,
            downloaded_bytes: downloaded,
            progress_pct: (pct * 10.0).round() / 10.0,
            status: self.status.read().clone(),
            error: self.error.read().clone(),
        }
    }
}

/// HuggingFace API client for searching & downloading GGUF models
pub struct HfClient {
    http: Client,
    downloads: DashMap<String, Arc<ActiveDownload>>,
}

impl HfClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent("squig-model-server/0.1")
                .build()
                .expect("Failed to build HTTP client"),
            downloads: DashMap::new(),
        }
    }

    /// Search HuggingFace for GGUF model repos.
    /// Uses the HF Hub API: GET /api/models?search=<query>&filter=gguf&sort=downloads&direction=-1
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<HfSearchResult>> {
        let url = format!(
            "https://huggingface.co/api/models?search={}&filter=gguf&sort=downloads&direction=-1&limit={}",
            urlencoding::encode(query),
            limit.min(50)
        );

        tracing::debug!("HF search: {}", url);

        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("HuggingFace API returned {}", resp.status());
        }

        let items: Vec<HfModelApiResponse> = resp.json().await?;

        let mut results = Vec::with_capacity(items.len());
        for item in items {
            // Filter to repos that actually contain GGUF files in their siblings
            let gguf_files: Vec<HfGgufFile> = item
                .siblings
                .unwrap_or_default()
                .into_iter()
                .filter(|s| s.rfilename.to_lowercase().ends_with(".gguf"))
                .map(|s| {
                    let size = s.size.unwrap_or(0);
                    HfGgufFile {
                        filename: s.rfilename,
                        size,
                        size_human: format_bytes(size),
                    }
                })
                .collect();

            if gguf_files.is_empty() {
                continue; // Skip repos with no GGUF files
            }

            results.push(HfSearchResult {
                repo_id: item.id,
                last_modified: item.last_modified.unwrap_or_default(),
                downloads: item.downloads.unwrap_or(0),
                likes: item.likes.unwrap_or(0),
                gguf_files,
                tags: item.tags.unwrap_or_default(),
            });
        }

        Ok(results)
    }

    /// Start downloading a GGUF file from HuggingFace.
    /// Returns immediately — the download runs in a background task.
    pub fn start_download(
        self: &Arc<Self>,
        repo_id: &str,
        filename: &str,
        dest_dir: &Path,
    ) -> Result<()> {
        let key = download_key(repo_id, filename);

        // Check if already downloading
        if let Some(existing) = self.downloads.get(&key) {
            let status = existing.status.read().clone();
            if status == DownloadStatus::Downloading || status == DownloadStatus::Queued {
                bail!("Already downloading {}/{}", repo_id, filename);
            }
            // If it was completed or failed, allow re-download
        }

        let active = Arc::new(ActiveDownload {
            repo_id: repo_id.to_string(),
            filename: filename.to_string(),
            total_bytes: AtomicU64::new(0),
            downloaded_bytes: AtomicU64::new(0),
            status: parking_lot::RwLock::new(DownloadStatus::Queued),
            error: parking_lot::RwLock::new(None),
            cancelled: AtomicBool::new(false),
        });

        self.downloads.insert(key, active.clone());

        let client = Arc::clone(self);
        let repo_id = repo_id.to_string();
        let filename = filename.to_string();
        let dest_dir = dest_dir.to_path_buf();

        tokio::spawn(async move {
            if let Err(e) = client
                .do_download(&repo_id, &filename, &dest_dir, &active)
                .await
            {
                tracing::error!("Download failed: {}/{}: {}", repo_id, filename, e);
                *active.status.write() = DownloadStatus::Failed;
                *active.error.write() = Some(e.to_string());
            }
        });

        Ok(())
    }

    /// Internal download implementation
    async fn do_download(
        &self,
        repo_id: &str,
        filename: &str,
        dest_dir: &Path,
        active: &ActiveDownload,
    ) -> Result<()> {
        *active.status.write() = DownloadStatus::Downloading;

        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo_id, filename
        );

        tracing::info!("Downloading {} -> {:?}", url, dest_dir);

        let resp = self
            .http
            .get(&url)
            .header("Accept", "application/octet-stream")
            .send()
            .await?;

        if !resp.status().is_success() {
            bail!("Download failed: HTTP {}", resp.status());
        }

        // Get content length for progress tracking
        let content_length = resp.content_length().unwrap_or(0);
        active.total_bytes.store(content_length, Ordering::Relaxed);

        // Ensure destination directory exists
        tokio::fs::create_dir_all(dest_dir).await?;

        let dest_path = dest_dir.join(filename);
        let temp_path = dest_dir.join(format!("{}.downloading", filename));

        let mut file = tokio::fs::File::create(&temp_path).await?;
        let mut stream = resp.bytes_stream();

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            if active.cancelled.load(Ordering::Relaxed) {
                // Clean up temp file
                let _ = tokio::fs::remove_file(&temp_path).await;
                *active.status.write() = DownloadStatus::Failed;
                *active.error.write() = Some("Download cancelled".to_string());
                return Ok(());
            }

            let chunk = chunk?;
            file.write_all(&chunk).await?;
            active
                .downloaded_bytes
                .fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }

        file.flush().await?;
        drop(file);

        // Rename temp file to final destination
        tokio::fs::rename(&temp_path, &dest_path).await?;

        *active.status.write() = DownloadStatus::Complete;
        tracing::info!("Download complete: {}", dest_path.display());

        Ok(())
    }

    /// Cancel an active download
    pub fn cancel_download(&self, repo_id: &str, filename: &str) -> Result<()> {
        let key = download_key(repo_id, filename);
        if let Some(dl) = self.downloads.get(&key) {
            dl.cancelled.store(true, Ordering::Relaxed);
            Ok(())
        } else {
            bail!("No active download for {}/{}", repo_id, filename)
        }
    }

    /// Get progress of all downloads
    pub fn download_progress(&self) -> Vec<DownloadProgress> {
        self.downloads
            .iter()
            .map(|entry| entry.value().snapshot())
            .collect()
    }

    /// Clean up completed/failed download entries
    pub fn clear_finished_downloads(&self) {
        self.downloads.retain(|_, v| {
            let status = v.status.read().clone();
            status == DownloadStatus::Queued || status == DownloadStatus::Downloading
        });
    }
}

/// HuggingFace API model response (partial)
#[derive(Debug, Deserialize)]
struct HfModelApiResponse {
    #[serde(rename = "modelId", alias = "id")]
    id: String,
    #[serde(rename = "lastModified")]
    last_modified: Option<String>,
    downloads: Option<u64>,
    likes: Option<u64>,
    tags: Option<Vec<String>>,
    siblings: Option<Vec<HfSibling>>,
}

/// A file within a HuggingFace model repo
#[derive(Debug, Deserialize)]
struct HfSibling {
    rfilename: String,
    size: Option<u64>,
}

fn download_key(repo_id: &str, filename: &str) -> String {
    format!("{}::{}", repo_id, filename)
}

fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    const KB: u64 = 1_024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else if bytes > 0 {
        format!("{} B", bytes)
    } else {
        "unknown".to_string()
    }
}
