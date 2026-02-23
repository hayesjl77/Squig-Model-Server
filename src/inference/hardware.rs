use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
pub struct HardwareInfo {
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub cpu_threads: usize,
    pub total_memory_gb: f64,
    pub available_memory_gb: f64,
    pub gpus: Vec<GpuInfo>,
    pub recommended_backend: String,
    pub has_vulkan: bool,
    pub has_cuda: bool,
    pub has_rocm: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub vram_mb: Option<u64>,
    pub driver: Option<String>,
}

impl HardwareInfo {
    pub fn summary(&self) -> String {
        let gpu_str = if self.gpus.is_empty() {
            "No GPU detected".to_string()
        } else {
            self.gpus
                .iter()
                .map(|g| g.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "{} ({} cores) | {:.1}GB RAM ({:.1}GB free) | {} | Backend: {}",
            self.cpu_name,
            self.cpu_cores,
            self.total_memory_gb,
            self.available_memory_gb,
            gpu_str,
            self.recommended_backend
        )
    }
}

/// Detect system hardware capabilities
pub fn detect_hardware() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let cpu_cores = sys.physical_core_count().unwrap_or(0);
    let cpu_threads = sys.cpus().len();
    let total_memory_gb = sys.total_memory() as f64 / 1_073_741_824.0;
    let available_memory_gb = sys.available_memory() as f64 / 1_073_741_824.0;

    // Detect GPUs via lspci or platform-specific methods
    let gpus = detect_gpus();

    // Determine recommended backend
    let has_vulkan = which::which("vulkaninfo").is_ok() || cfg!(target_os = "linux");
    let has_cuda = which::which("nvidia-smi").is_ok();
    let has_rocm = which::which("rocminfo").is_ok()
        || std::path::Path::new("/opt/rocm").exists();

    let recommended_backend = if has_cuda {
        "cuda".to_string()
    } else if has_rocm {
        "rocm".to_string()
    } else if has_vulkan {
        "vulkan".to_string()
    } else {
        "cpu".to_string()
    };

    HardwareInfo {
        cpu_name,
        cpu_cores,
        cpu_threads,
        total_memory_gb,
        available_memory_gb,
        gpus,
        recommended_backend,
        has_vulkan,
        has_cuda,
        has_rocm,
    }
}

/// Detect GPUs on the system
fn detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    // Try nvidia-smi first
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .arg("--query-gpu=name,memory.total,driver_version")
        .arg("--format=csv,noheader,nounits")
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 3 {
                    gpus.push(GpuInfo {
                        name: parts[0].to_string(),
                        vendor: "NVIDIA".to_string(),
                        vram_mb: parts[1].parse().ok(),
                        driver: Some(parts[2].to_string()),
                    });
                }
            }
        }
    }

    // Try lspci for AMD GPUs
    if let Ok(output) = std::process::Command::new("lspci").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let lower = line.to_lowercase();
                if (lower.contains("vga") || lower.contains("3d controller"))
                    && lower.contains("amd")
                    && !gpus.iter().any(|g| line.contains(&g.name))
                {
                    // Extract GPU name from lspci output
                    let name = line
                        .split(':')
                        .last()
                        .unwrap_or("AMD GPU")
                        .trim()
                        .to_string();
                    gpus.push(GpuInfo {
                        name,
                        vendor: "AMD".to_string(),
                        vram_mb: None,
                        driver: None,
                    });
                }
            }
        }
    }

    // Windows: try DXGI via powershell (fallback)
    #[cfg(target_os = "windows")]
    {
        if gpus.is_empty() {
            if let Ok(output) = std::process::Command::new("powershell")
                .args([
                    "-Command",
                    "Get-CimInstance Win32_VideoController | Select-Object Name, AdapterRAM, DriverVersion | ConvertTo-Json",
                ])
                .output()
            {
                if output.status.success() {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        let items = match &val {
                            serde_json::Value::Array(arr) => arr.clone(),
                            obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                            _ => vec![],
                        };
                        for item in items {
                            gpus.push(GpuInfo {
                                name: item["Name"].as_str().unwrap_or("Unknown").to_string(),
                                vendor: "Unknown".to_string(),
                                vram_mb: item["AdapterRAM"]
                                    .as_u64()
                                    .map(|b| b / 1_048_576),
                                driver: item["DriverVersion"]
                                    .as_str()
                                    .map(|s| s.to_string()),
                            });
                        }
                    }
                }
            }
        }
    }

    gpus
}
