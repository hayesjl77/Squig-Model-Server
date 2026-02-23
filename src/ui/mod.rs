use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
};
use rust_embed::Embed;

/// Embedded UI assets (built from the ui/ Svelte project)
/// In development, falls back to serving from the filesystem.
#[derive(Embed)]
#[folder = "ui/dist/"]
struct UiAssets;

/// Serve embedded static files, with index.html as fallback for SPA routing
pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try exact file match first
    if !path.is_empty() {
        if let Some(file) = UiAssets::get(path) {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, "public, max-age=3600")
                .body(axum::body::Body::from(file.data.to_vec()))
                .unwrap();
        }
    }

    // SPA fallback: serve index.html for any unknown route
    match UiAssets::get("index.html") {
        Some(file) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap(),
        None => {
            // UI not built yet - serve a helpful placeholder
            Html(PLACEHOLDER_HTML).into_response()
        }
    }
}

const PLACEHOLDER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Squig Model Server</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, monospace;
            background: #0a0a0f;
            color: #e0e0e0;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
        }
        .container {
            text-align: center;
            padding: 2rem;
            max-width: 600px;
        }
        h1 { color: #6ee7b7; font-size: 2rem; margin-bottom: 1rem; }
        p { color: #888; margin-bottom: 0.5rem; line-height: 1.6; }
        code {
            background: #1a1a2e;
            padding: 0.2rem 0.5rem;
            border-radius: 4px;
            color: #6ee7b7;
            font-size: 0.9rem;
        }
        .status { margin-top: 2rem; }
        .endpoint {
            background: #111;
            border: 1px solid #222;
            border-radius: 8px;
            padding: 1rem;
            margin: 0.5rem 0;
            text-align: left;
        }
        .endpoint .method { color: #6ee7b7; font-weight: bold; }
        .endpoint .path { color: #e0e0e0; }
        a { color: #6ee7b7; }
    </style>
</head>
<body>
    <div class="container">
        <h1>⚡ Squig Model Server</h1>
        <p>The API is running. The dashboard UI hasn't been built yet.</p>
        <p>Build it with: <code>cd ui && npm install && npm run build</code></p>

        <div class="status">
            <h2 style="color: #6ee7b7; margin-bottom: 1rem;">API Endpoints</h2>
            <div class="endpoint">
                <span class="method">GET</span>
                <span class="path"> <a href="/api/status">/api/status</a> — Server status</span>
            </div>
            <div class="endpoint">
                <span class="method">GET</span>
                <span class="path"> <a href="/api/hardware">/api/hardware</a> — Hardware info</span>
            </div>
            <div class="endpoint">
                <span class="method">GET</span>
                <span class="path"> <a href="/api/models/available">/api/models/available</a> — Available models</span>
            </div>
            <div class="endpoint">
                <span class="method">GET</span>
                <span class="path"> <a href="/v1/models">/v1/models</a> — Models (OpenAI)</span>
            </div>
            <div class="endpoint">
                <span class="method">POST</span>
                <span class="path"> /v1/chat/completions — Chat (OpenAI)</span>
            </div>
        </div>
    </div>
</body>
</html>"#;
