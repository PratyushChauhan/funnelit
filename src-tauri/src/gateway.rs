//! Authenticated Streamable HTTP MCP gateway with three meta-tools.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Router,
    extract::Request,
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    model::CallToolResult,
    schemars, tool, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::RwLock, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    config::{self, AppConfig, BIND_ADDR, ENDPOINT_URL, McpServer, redact},
    pool::SharedPool,
};

#[derive(Clone)]
pub struct AppInner {
    pub dir: std::path::PathBuf,
    pub config: Arc<RwLock<AppConfig>>,
    pub pool: SharedPool,
    pub token: Arc<RwLock<String>>,
    pub running: Arc<AtomicBool>,
    shutdown: Arc<RwLock<Option<CancellationToken>>>,
    server_task: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl AppInner {
    /// Inputs: config directory. Outputs: initialized app state.
    pub fn new(dir: std::path::PathBuf) -> Result<Self, config::ConfigError> {
        let cfg = config::load_config(&dir)?;
        let token = config::ensure_endpoint_token()?;
        Ok(Self {
            dir,
            config: Arc::new(RwLock::new(cfg)),
            pool: Arc::new(crate::pool::ClientPool::new()),
            token: Arc::new(RwLock::new(token)),
            running: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(RwLock::new(None)),
            server_task: Arc::new(RwLock::new(None)),
        })
    }

    /// Inputs: none. Outputs: server matching id, if present.
    pub async fn find(&self, id: &str) -> Option<McpServer> {
        self.config
            .read()
            .await
            .servers
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }
}

#[derive(Clone)]
struct Gateway {
    app: Arc<AppInner>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpIdArgs {
    /// Stable MCP identifier.
    mcp_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExecuteArgs {
    mcp_id: String,
    tool_name: String,
    #[serde(default)]
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpInfo {
    id: String,
    name: String,
    transport: String,
    enabled: bool,
}

#[tool_router(server_handler)]
impl Gateway {
    /// Inputs: none. Outputs: configured MCP summaries.
    #[tool(description = "List configured MCP servers available through Funnelit")]
    async fn list_mcps(&self) -> Result<Json<Vec<McpInfo>>, String> {
        let cfg = self.app.config.read().await;
        Ok(Json(
            cfg.servers
                .iter()
                .map(|s| McpInfo {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    transport: match s.transport {
                        config::McpTransport::Stdio { .. } => "stdio".into(),
                        config::McpTransport::Http { .. } => "http".into(),
                    },
                    enabled: s.enabled,
                })
                .collect(),
        ))
    }

    /// Inputs: mcp_id. Outputs: upstream tool names, descriptions, and schemas.
    #[tool(description = "List tools exposed by a configured MCP server")]
    async fn list_mcp_tools(
        &self,
        Parameters(McpIdArgs { mcp_id }): Parameters<McpIdArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let Some(server) = self.app.find(&mcp_id).await else {
            return Ok(CallToolResult::structured_error(serde_json::json!({
                "error": "unknown mcp_id"
            })));
        };
        match self.app.pool.list_tools(&server).await {
            Ok(tools) => Ok(CallToolResult::structured(
                serde_json::to_value(tools).unwrap_or_default(),
            )),
            Err(e) => Ok(CallToolResult::structured_error(serde_json::json!({
                "error": redact(&e.to_string())
            }))),
        }
    }

    /// Inputs: mcp_id, tool_name, arguments. Outputs: upstream CallToolResult.
    #[tool(description = "Execute a tool on a configured MCP server")]
    async fn execute_mcp_tool(
        &self,
        Parameters(ExecuteArgs {
            mcp_id,
            tool_name,
            arguments,
        }): Parameters<ExecuteArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let Some(server) = self.app.find(&mcp_id).await else {
            return Ok(CallToolResult::structured_error(serde_json::json!({
                "error": "unknown mcp_id"
            })));
        };
        match self.app.pool.call_tool(&server, &tool_name, arguments).await {
            Ok(result) => Ok(result),
            Err(e) => Ok(CallToolResult::structured_error(serde_json::json!({
                "error": redact(&e.to_string())
            }))),
        }
    }
}

/// Inputs: request headers and expected token. Outputs: true when Authorization matches.
fn bearer_ok(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
                .is_some_and(|t| t == expected)
        })
}

async fn auth_middleware(
    axum::extract::State(app): axum::extract::State<Arc<AppInner>>,
    req: Request,
    next: Next,
) -> Response {
    if req.headers().contains_key(header::ORIGIN) {
        return (
            StatusCode::FORBIDDEN,
            "browser Origin requests are not allowed",
        )
            .into_response();
    }
    let token = app.token.read().await.clone();
    if !bearer_ok(req.headers(), &token) {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "unauthorized",
        )
            .into_response();
    }
    next.run(req).await
}

/// Inputs: shared app state. Outputs: Ok(()) when the funnel HTTP server is listening.
pub async fn start(app: Arc<AppInner>) -> Result<(), String> {
    if app.running.load(Ordering::SeqCst) {
        return Ok(());
    }
    let ct = CancellationToken::new();
    let child = ct.child_token();
    let gateway = Gateway { app: app.clone() };
    let service = StreamableHttpService::new(
        {
            let gateway = gateway.clone();
            move || Ok(gateway.clone())
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_cancellation_token(child)
            .with_allowed_hosts(["127.0.0.1", "localhost", "127.0.0.1:7341", "localhost:7341"]),
    );

    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(app.clone(), auth_middleware));

    let listener = TcpListener::bind(BIND_ADDR)
        .await
        .map_err(|e| e.to_string())?;
    let shutdown = ct.clone();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown.cancelled().await;
            })
            .await;
    });

    *app.shutdown.write().await = Some(ct);
    *app.server_task.write().await = Some(task);
    app.running.store(true, Ordering::SeqCst);
    Ok(())
}

/// Inputs: shared app state. Outputs: Ok(()) after stopping HTTP and clearing clients.
pub async fn stop(app: Arc<AppInner>) -> Result<(), String> {
    if let Some(ct) = app.shutdown.write().await.take() {
        ct.cancel();
    }
    if let Some(task) = app.server_task.write().await.take() {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(3), task).await;
    }
    app.pool.clear().await;
    app.running.store(false, Ordering::SeqCst);
    Ok(())
}

/// Inputs: none. Outputs: funnel public URL constant.
pub fn endpoint_url() -> &'static str {
    ENDPOINT_URL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_matching_is_exact() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer secret-token".parse().unwrap());
        assert!(bearer_ok(&headers, "secret-token"));
        assert!(!bearer_ok(&headers, "other"));
    }
}
