use axum::{extract::State, response::Json};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::server::AppState;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WebSearchRequest {
    pub query: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

fn default_max_results() -> usize {
    5
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Serialize)]
pub struct WebSearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub error: Option<String>,
}

// ─── Handler ─────────────────────────────────────────────────────────────────

/// POST /api/web-search
pub async fn web_search(
    State(_state): State<AppState>,
    Json(request): Json<WebSearchRequest>,
) -> Json<WebSearchResponse> {
    info!("Web search request: {:?}", request.query);

    match fetch_duckduckgo_results(&request.query, request.max_results).await {
        Ok(results) => {
            info!("Web search returned {} results", results.len());
            Json(WebSearchResponse {
                query: request.query,
                results,
                error: None,
            })
        }
        Err(e) => {
            warn!("Web search failed: {}", e);
            Json(WebSearchResponse {
                query: request.query,
                results: vec![],
                error: Some(e.to_string()),
            })
        }
    }
}

// ─── DuckDuckGo Scraping ─────────────────────────────────────────────────────

async fn fetch_duckduckgo_results(
    query: &str,
    max: usize,
) -> anyhow::Result<Vec<SearchResult>> {
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
        )
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );

    let html = client
        .get(&url)
        .header("Accept", "text/html")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?
        .text()
        .await?;

    parse_duckduckgo_html(&html, max)
}

fn parse_duckduckgo_html(html: &str, max: usize) -> anyhow::Result<Vec<SearchResult>> {
    let mut results = Vec::new();

    // Extract title + URL from <a class="result__a" href="...uddg=ENCODED_URL...">Title</a>
    let link_re = Regex::new(
        r#"class="result__a"\s+href="([^"]+)"[^>]*>([\s\S]*?)</a>"#,
    )?;

    // Extract snippet from <a class="result__snippet" ...>Snippet</a>
    let snippet_re = Regex::new(
        r#"class="result__snippet"[^>]*>([\s\S]*?)</a>"#,
    )?;

    let links: Vec<_> = link_re.captures_iter(html).collect();
    let snippets: Vec<_> = snippet_re.captures_iter(html).collect();

    let count = links.len().min(snippets.len()).min(max);

    for i in 0..count {
        let raw_url = &links[i][1];
        let raw_title = &links[i][2];
        let raw_snippet = &snippets[i][1];

        let url = extract_ddg_url(raw_url);
        let title = clean_html(raw_title);
        let snippet = clean_html(raw_snippet);

        // Skip empty or ad results
        if url.is_empty() || title.is_empty() {
            continue;
        }
        // Skip DDG internal links
        if url.starts_with("https://duckduckgo.com") {
            continue;
        }

        results.push(SearchResult {
            title,
            url,
            snippet,
        });
    }

    Ok(results)
}

/// Extract the actual destination URL from DuckDuckGo's redirect wrapper.
/// DDG wraps URLs like: //duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=...
fn extract_ddg_url(raw: &str) -> String {
    if let Some(pos) = raw.find("uddg=") {
        let after = &raw[pos + 5..];
        let encoded = if let Some(amp) = after.find('&') {
            &after[..amp]
        } else {
            after
        };
        // URL-decode the destination
        urlencoding::decode(encoded)
            .map(|s| s.into_owned())
            .unwrap_or_else(|_| encoded.to_string())
    } else {
        // Direct URL (no redirect wrapper)
        let url = raw.trim_start_matches("//");
        if !url.starts_with("http") {
            format!("https://{}", url)
        } else {
            url.to_string()
        }
    }
}

/// Strip HTML tags and decode common entities.
fn clean_html(html: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    let text = tag_re.replace_all(html, "");
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ddg_url() {
        let raw = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fdoc.rust-lang.org%2Fbook%2F&rut=abc";
        assert_eq!(
            extract_ddg_url(raw),
            "https://doc.rust-lang.org/book/"
        );
    }

    #[test]
    fn test_clean_html() {
        let html = "Hello <b>world</b> &amp; friends";
        assert_eq!(clean_html(html), "Hello world & friends");
    }
}
