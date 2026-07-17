//! Local static VitePress docs server opened from the Docs button.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
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

/// Inputs: none. Outputs: true when HTTP GET / returns any response.
async fn docs_reachable() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_millis(400))
        .build()
    else {
        return false;
    };
    client
        .get(URL)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Inputs: app handle. Outputs: docs URL after ensuring the local server is listening.
///
/// Uses a dedicated OS thread + Tokio runtime: spawning on Tauri's runtime accepted
/// TCP but never served HTTP in practice.
pub async fn ensure_and_open(app: &AppHandle) -> Result<String, String> {
    let root = resolve_docs_root(app)?;
    if !STARTED.swap(true, Ordering::SeqCst) {
        let root_thread = root.clone();
        thread::Builder::new()
            .name("sumeru-docs".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(err) => {
                        eprintln!("sumeru docs runtime: {err}");
                        STARTED.store(false, Ordering::SeqCst);
                        return;
                    }
                };
                rt.block_on(async move {
                    let listener = match tokio::net::TcpListener::bind(BIND).await {
                        Ok(l) => l,
                        Err(err) => {
                            eprintln!("sumeru docs bind {BIND}: {err}");
                            STARTED.store(false, Ordering::SeqCst);
                            return;
                        }
                    };
                    let router = Router::new().fallback_service(
                        ServeDir::new(root_thread).append_index_html_on_directories(true),
                    );
                    if let Err(err) = axum::serve(listener, router).await {
                        eprintln!("sumeru docs server exited: {err}");
                        STARTED.store(false, Ordering::SeqCst);
                    }
                });
            })
            .map_err(|e| {
                STARTED.store(false, Ordering::SeqCst);
                format!("docs server thread: {e}")
            })?;

        for _ in 0..40 {
            if docs_reachable().await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if !docs_reachable().await {
            STARTED.store(false, Ordering::SeqCst);
            return Err(format!(
                "docs server started but {URL} is not responding (root {})",
                root.display()
            ));
        }
    }
    open::that(URL).map_err(|e| e.to_string())?;
    Ok(URL.to_string())
}
