use std::path::PathBuf;
use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Start the Axum backend server on a background thread.
fn start_backend_server(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async move {
            // Find config.toml
            let config_path = find_config();

            let cfg = squig_model_server::config::ServerConfig::load(&config_path)
                .unwrap_or_else(|_| {
                    tracing::warn!("Could not load config from {:?}, using defaults", config_path);
                    squig_model_server::config::ServerConfig::default()
                });

            tracing::info!(
                "Starting backend on {}:{}",
                cfg.server.host,
                cfg.server.port
            );

            // Run the server — blocks until shutdown
            if let Err(e) = squig_model_server::server::run(cfg, false).await {
                tracing::error!("Backend server error: {}", e);
                let _ = app_handle.emit("backend-error", format!("{}", e));
            }
        });
    });
}

/// Find config.toml in likely locations
fn find_config() -> PathBuf {
    // 1. Next to the executable
    if let Ok(exe) = std::env::current_exe() {
        let beside_exe = exe.parent().unwrap_or(&exe).join("config.toml");
        if beside_exe.exists() {
            return beside_exe;
        }
    }

    // 2. Current working directory
    let cwd = PathBuf::from("config.toml");
    if cwd.exists() {
        return cwd;
    }

    // 3. User config dir (~/.config/squig-model-server/config.toml)
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

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();

            // Start the Axum backend server
            start_backend_server(handle);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Squig Model Server");
}
