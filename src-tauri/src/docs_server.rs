//! Local static VitePress docs server opened from the Docs button.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use axum::Router;
use tauri::{path::BaseDirectory, AppHandle, Manager};
use tower_http::services::ServeDir;

const BIND: &str = "127.0.0.1:7343";
const URL: &str = "http://127.0.0.1:7343/";

static STARTED: AtomicBool = AtomicBool::new(false);

/// Inputs: app handle. Outputs: path to built VitePress `index.html` directory.
fn resolve_docs_root(app: &AppHandle) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    for rel in ["docs", "resources/docs"] {
        if let Ok(dir) = app.path().resolve(rel, BaseDirectory::Resource) {
            candidates.push(dir);
        }
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/docs"));
    for dir in candidates {
        if dir.join("index.html").is_file() {
            return Ok(dir);
        }
    }
    Err(
        "docs not built — run `npm run docs:build` (included in `tauri dev` / `tauri build`)"
            .into(),
    )
}

/// Inputs: app handle. Outputs: docs URL after ensuring the local server is listening.
pub async fn ensure_and_open(app: &AppHandle) -> Result<String, String> {
    let root = resolve_docs_root(app)?;
    if !STARTED.swap(true, Ordering::SeqCst) {
        let listener = tokio::net::TcpListener::bind(BIND)
            .await
            .map_err(|e| {
                STARTED.store(false, Ordering::SeqCst);
                format!("docs server bind {BIND}: {e}")
            })?;
        let router = Router::new().fallback_service(
            ServeDir::new(root).append_index_html_on_directories(true),
        );
        // Same pattern as the MCP gateway — tokio::spawn on the command runtime.
        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, router).await {
                eprintln!("funnelit docs server exited: {err}");
                STARTED.store(false, Ordering::SeqCst);
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    open::that(URL).map_err(|e| e.to_string())?;
    Ok(URL.to_string())
}
