use axum::{extract::State, Json};

use crate::server::AppState;

/// GET /api/health
pub async fn health_check(State(state): State<AppState>) -> Json<serde_json::Value> {
    let uptime = chrono::Utc::now() - state.start_time;
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": uptime.num_seconds(),
    }))
}
