mod config;
mod gateway;
mod pool;

use std::sync::Arc;

use config::{
    McpServer, McpTransport, SecretMap, delete_secrets, redact, rotate_endpoint_token, save_config,
    store_secrets, validate_server,
};
use gateway::{AppInner, endpoint_url};
use serde::Serialize;
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone)]
struct State(Arc<AppInner>);

#[derive(Serialize)]
struct StatusDto {
    running: bool,
    endpoint: String,
}

#[derive(Serialize)]
struct ServerDto {
    id: String,
    name: String,
    enabled: bool,
    transport: McpTransport,
}

/// Inputs: app state. Outputs: funnel running flag and endpoint URL.
#[tauri::command]
async fn get_status(state: tauri::State<'_, State>) -> Result<StatusDto, String> {
    Ok(StatusDto {
        running: state.0.running.load(std::sync::atomic::Ordering::SeqCst),
        endpoint: endpoint_url().into(),
    })
}

/// Inputs: app state. Outputs: configured MCP servers (no secret values).
#[tauri::command]
async fn list_servers(state: tauri::State<'_, State>) -> Result<Vec<ServerDto>, String> {
    let cfg = state.0.config.read().await;
    Ok(cfg
        .servers
        .iter()
        .map(|s| ServerDto {
            id: s.id.clone(),
            name: s.name.clone(),
            enabled: s.enabled,
            transport: s.transport.clone(),
        })
        .collect())
}

/// Inputs: optional id, name, enabled, transport, secrets. Outputs: saved server id.
#[tauri::command]
async fn upsert_server(
    state: tauri::State<'_, State>,
    id: Option<String>,
    name: String,
    enabled: bool,
    transport: McpTransport,
    secrets: SecretMap,
) -> Result<String, String> {
    let id = id.filter(|s| !s.is_empty()).unwrap_or_else(|| Uuid::new_v4().to_string());
    let mut transport = transport;
    if let McpTransport::Http { has_bearer, .. } = &mut transport {
        let keep_bearer = !*has_bearer
            && secrets.bearer.as_ref().is_none_or(|b| b.is_empty())
            && config::get_bearer_secret(&id).is_some();
        if keep_bearer {
            *has_bearer = true;
        }
    }
    let server = McpServer {
        id: id.clone(),
        name,
        enabled,
        transport,
    };
    validate_server(&server).map_err(|e| e.to_string())?;
    store_secrets(&id, &secrets).map_err(|e| e.to_string())?;

    let mut cfg = state.0.config.write().await;
    if let Some(existing) = cfg.servers.iter_mut().find(|s| s.id == id) {
        state.0.pool.invalidate(&id).await;
        *existing = server;
    } else {
        cfg.servers.push(server);
    }
    save_config(&state.0.dir, &cfg).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Inputs: mcp id. Outputs: unit after removal.
#[tauri::command]
async fn remove_server(state: tauri::State<'_, State>, id: String) -> Result<(), String> {
    let mut cfg = state.0.config.write().await;
    let Some(pos) = cfg.servers.iter().position(|s| s.id == id) else {
        return Err("unknown mcp".into());
    };
    let removed = cfg.servers.remove(pos);
    match &removed.transport {
        McpTransport::Stdio { env_keys, .. } => delete_secrets(&id, env_keys, &[], false),
        McpTransport::Http {
            header_keys,
            has_bearer,
            ..
        } => delete_secrets(&id, &[], header_keys, *has_bearer),
    }
    state.0.pool.invalidate(&id).await;
    save_config(&state.0.dir, &cfg).map_err(|e| e.to_string())?;
    Ok(())
}

/// Inputs: none. Outputs: unit when funnel is listening.
#[tauri::command]
async fn start_funnel(state: tauri::State<'_, State>) -> Result<(), String> {
    gateway::start(state.0.clone()).await
}

/// Inputs: none. Outputs: unit when funnel has stopped.
#[tauri::command]
async fn stop_funnel(state: tauri::State<'_, State>) -> Result<(), String> {
    gateway::stop(state.0.clone()).await
}

/// Inputs: none. Outputs: current bearer token for client config.
#[tauri::command]
async fn get_token(state: tauri::State<'_, State>) -> Result<String, String> {
    Ok(state.0.token.read().await.clone())
}

/// Inputs: none. Outputs: newly rotated bearer token.
#[tauri::command]
async fn rotate_token(state: tauri::State<'_, State>) -> Result<String, String> {
    let token = rotate_endpoint_token().map_err(|e| e.to_string())?;
    *state.0.token.write().await = token.clone();
    Ok(token)
}

/// Inputs: mcp id. Outputs: Ok message or connection error.
#[tauri::command]
async fn test_server(state: tauri::State<'_, State>, id: String) -> Result<String, String> {
    let server = state
        .0
        .find(&id)
        .await
        .ok_or_else(|| "unknown mcp".to_string())?;
    let tools = state
        .0
        .pool
        .list_tools(&server)
        .await
        .map_err(|e| redact(&e.to_string()))?;
    Ok(format!("connected ({} tools)", tools.len()))
}

/// Inputs: unsaved transport + secrets. Outputs: Ok message when a temporary connect works.
#[tauri::command]
async fn test_draft(
    state: tauri::State<'_, State>,
    name: String,
    transport: McpTransport,
    secrets: SecretMap,
) -> Result<String, String> {
    let id = format!("draft-{}", Uuid::new_v4());
    let server = McpServer {
        id: id.clone(),
        name,
        enabled: true,
        transport: transport.clone(),
    };
    validate_server(&server).map_err(|e| e.to_string())?;
    store_secrets(&id, &secrets).map_err(|e| e.to_string())?;
    let result = state.0.pool.list_tools(&server).await;
    state.0.pool.invalidate(&id).await;
    match &transport {
        McpTransport::Stdio { env_keys, .. } => delete_secrets(&id, env_keys, &[], false),
        McpTransport::Http {
            header_keys,
            has_bearer,
            ..
        } => delete_secrets(&id, &[], header_keys, *has_bearer),
    }
    result
        .map(|t| format!("connected ({} tools)", t.len()))
        .map_err(|e| redact(&e.to_string()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app
                .path()
                .app_config_dir()
                .expect("app config dir")
                .join("funnelit");
            std::fs::create_dir_all(&dir)?;
            let inner = AppInner::new(dir).map_err(|e| e.to_string())?;
            app.manage(State(Arc::new(inner)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            list_servers,
            upsert_server,
            remove_server,
            start_funnel,
            stop_funnel,
            get_token,
            rotate_token,
            test_server,
            test_draft,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = &event {
                api.prevent_exit();
                let handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(state) = handle.try_state::<State>() {
                        let _ = gateway::stop(state.0.clone()).await;
                    }
                    handle.exit(0);
                });
            }
        });
}
