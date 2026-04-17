use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use squig_model_server::{config, inference, server};

#[derive(Parser, Debug)]
#[command(name = "squig-model-server", version, about = "High-performance local LLM model server")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Host to bind to
    #[arg(long)]
    host: Option<String>,

    /// Port to bind to
    #[arg(short, long)]
    port: Option<u16>,

    /// Path to models directory
    #[arg(short, long)]
    models_dir: Option<PathBuf>,

    /// Open dashboard in browser on start
    #[arg(long, default_value = "true")]
    open_browser: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new(&cli.log_level)
        }))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    // Load configuration
    let mut cfg = config::ServerConfig::load(&cli.config).unwrap_or_default();

    // CLI overrides
    if let Some(host) = cli.host {
        cfg.server.host = host;
    }
    if let Some(port) = cli.port {
        cfg.server.port = port;
    }
    if let Some(models_dir) = cli.models_dir {
        cfg.models.directories = vec![models_dir];
    }

    // Print banner
    print_banner(&cfg);

    // Detect hardware
    let hw = inference::detect_hardware();
    tracing::info!("Hardware: {}", hw.summary());

    // Start server
    server::run(cfg, cli.open_browser).await
}

fn print_banner(cfg: &config::ServerConfig) {
    println!();
    println!("{}", "╔══════════════════════════════════════════╗".bright_cyan());
    println!("{}", "║       Squig Model Server  v0.2.0        ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════════════╝".bright_cyan());
    println!();
    println!(
        "  {} http://{}:{}",
        "Dashboard:".bright_green(),
        cfg.server.host,
        cfg.server.port
    );
    println!(
        "  {} http://{}:{}/v1",
        "API:      ".bright_green(),
        cfg.server.host,
        cfg.server.port
    );
    println!();
}
