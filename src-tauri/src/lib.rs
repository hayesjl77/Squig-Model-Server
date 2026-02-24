use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Find config.toml in likely locations
fn find_config() -> PathBuf {
    // 1. Next to the executable
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap_or(&exe);
        let beside_exe = exe_dir.join("config.toml");
        if beside_exe.exists() {
            return beside_exe;
        }
        // Parent of exe dir (target/debug/../)
        if let Some(parent) = exe_dir.parent() {
            let p = parent.join("config.toml");
            if p.exists() {
                return p;
            }
        }
    }

    // 2. Current working directory
    let cwd = PathBuf::from("config.toml");
    if cwd.exists() {
        return cwd;
    }

    // 3. Parent of CWD (handles src-tauri/ case in dev mode)
    let parent_cwd = PathBuf::from("../config.toml");
    if parent_cwd.exists() {
        return parent_cwd.canonicalize().unwrap_or(parent_cwd);
    }

    // 4. User config dir (~/.config/squig-model-server/config.toml)
    if let Some(config_dir) = dirs::config_dir() {
        let user_config = config_dir.join("squig-model-server").join("config.toml");
        if user_config.exists() {
            return user_config;
        }
    }

    // Fallback
    PathBuf::from("config.toml")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    let _ = tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .try_init();

    // Load config to get port
    let config_path = find_config();
    let cfg = squig_model_server::config::ServerConfig::load(&config_path)
        .unwrap_or_else(|_| {
            tracing::warn!("Could not load config from {:?}, using defaults", config_path);
            squig_model_server::config::ServerConfig::default()
        });
    let port = cfg.server.port;

    // Start the Axum backend on a background thread
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async move {
            tracing::info!("Starting backend on {}:{}", cfg.server.host, cfg.server.port);
            if let Err(e) = squig_model_server::server::run(cfg, false).await {
                tracing::error!("Backend server error: {}", e);
            }
        });
    });

    // Wait for the backend to be ready (blocks main thread briefly at startup)
    tracing::info!("Waiting for backend to start...");
    let addr = format!("127.0.0.1:{}", port);
    for attempt in 1..=120 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        if std::net::TcpStream::connect(&addr).is_ok() {
            tracing::info!("Backend ready after {} attempts", attempt);
            break;
        }
    }

    // Build Tauri app — window will load straight from the running backend
    let backend_url: tauri::Url = format!("http://127.0.0.1:{}", port).parse().unwrap();

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            // Create the window pointing directly at the backend dashboard
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(backend_url),
            )
            .title("Squig Model Server")
            .inner_size(1400.0, 900.0)
            .min_inner_size(1024.0, 700.0)
            .center()
            .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Squig Model Server");
}
