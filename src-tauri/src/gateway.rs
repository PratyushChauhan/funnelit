//! Authenticated Streamable HTTP MCP gateway with three meta-tools.

use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    extract::Request,
    http::{header, HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Router,
};
use tokio_stream::StreamExt;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{
        CallToolResult, Implementation, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ServiceExt,
};
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{
    config::{self, redact, AppConfig, McpServer, BIND_ADDR, ENDPOINT_URL},
    pool::SharedPool,
};

#[derive(Clone)]
pub struct AppInner {
    pub dir: std::path::PathBuf,
    pub config: Arc<RwLock<AppConfig>>,
    pub pool: SharedPool,
    pub token: Arc<RwLock<String>>,
    pub running: Arc<AtomicBool>,
    pub exiting: Arc<AtomicBool>,
    lifecycle: Arc<Mutex<()>>,
    shutdown: Arc<RwLock<Option<CancellationToken>>>,
    server_task: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl AppInner {
    /// Inputs: config directory and optional HTTP bearer. Outputs: app state.
    fn with_token(
        dir: std::path::PathBuf,
        token: Option<String>,
    ) -> Result<Self, config::ConfigError> {
        let cfg = config::load_config(&dir)?;
        Ok(Self {
            dir,
            config: Arc::new(RwLock::new(cfg)),
            pool: Arc::new(crate::pool::ClientPool::new()),
            token: Arc::new(RwLock::new(token.unwrap_or_default())),
            running: Arc::new(AtomicBool::new(false)),
            exiting: Arc::new(AtomicBool::new(false)),
            lifecycle: Arc::new(Mutex::new(())),
            shutdown: Arc::new(RwLock::new(None)),
            server_task: Arc::new(RwLock::new(None)),
        })
    }

    /// Inputs: config directory. Outputs: app state with keyring endpoint token.
    pub fn new(dir: std::path::PathBuf) -> Result<Self, config::ConfigError> {
        Self::with_token(dir, Some(config::ensure_endpoint_token()?))
    }

    /// Inputs: config directory. Outputs: app state for stdio (no keyring token).
    pub fn for_stdio(dir: std::path::PathBuf) -> Result<Self, config::ConfigError> {
        Self::with_token(dir, None)
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

#[derive(Debug, Serialize)]
struct McpInfo {
    id: String,
    name: String,
    transport: String,
    enabled: bool,
}

#[tool_router]
impl Gateway {
    /// Inputs: none. Outputs: configured MCP summaries.
    #[tool(description = "List configured MCP servers available through Funnelit")]
    async fn list_mcps(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let cfg = self.app.config.read().await;
        let infos: Vec<McpInfo> = cfg
            .servers
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
            .collect();
        // Cursor validates structuredContent as a JSON object (record), not an array.
        let structured = serde_json::json!({ "mcps": infos });
        Ok(CallToolResult::structured(structured))
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
            Ok(tools) => {
                // Cursor validates structuredContent as a JSON object (record), not an array.
                let structured = serde_json::json!({ "tools": tools });
                Ok(CallToolResult::structured(structured))
            }
            Err(e) => {
                Ok(CallToolResult::structured_error(serde_json::json!({
                    "error": redact(&e.to_string())
                })))
            }
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
        match self
            .app
            .pool
            .call_tool(&server, &tool_name, arguments)
            .await
        {
            Ok(result) => Ok(cursor_safe_tool_result(result)),
            Err(e) => {
                Ok(CallToolResult::structured_error(serde_json::json!({
                    "error": redact(&e.to_string())
                })))
            }
        }
    }
}

/// Inputs: upstream CallToolResult. Outputs: same with object-only structuredContent.
fn cursor_safe_tool_result(mut result: CallToolResult) -> CallToolResult {
    if let Some(sc) = result.structured_content.take() {
        result.structured_content = Some(match sc {
            serde_json::Value::Object(_) => sc,
            other => serde_json::json!({ "result": other }),
        });
    }
    result
}

impl Gateway {
    /// Inputs: none. Outputs: gateway tool definitions after router schema build.
    fn list_gateway_tools() -> Vec<rmcp::model::Tool> {
        Self::tool_router().list_all()
    }
}

#[tool_handler(router = Self::tool_router())]
impl rmcp::ServerHandler for Gateway {
    /// Inputs: none. Outputs: server capabilities and implementation info.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "funnelit",
                env!("CARGO_PKG_VERSION"),
            ))
    }

    /// Inputs: pagination params and request context. Outputs: gateway tool list.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let tools = Self::list_gateway_tools();
        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }
}

/// Inputs: two strings. Outputs: true when equal in constant time for equal lengths.
fn ct_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Inputs: request headers and expected token. Outputs: true when Authorization matches.
fn bearer_ok(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .is_some_and(|t| ct_eq(t, expected))
}

/// Inputs: app state and request. Outputs: 403/401 or the downstream response.
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

/// Soft GET SSE for Cursor peers (rmcp stateless would return 405; POST stays JSON).
///
/// Inputs: request.
/// Outputs: keep-alive SSE for GET, else next middleware.
async fn soft_get_sse(req: Request, next: Next) -> Response {
    if req.method() != Method::GET {
        return next.run(req).await;
    }
    // pending() keeps the stream open so KeepAlive pings continue (avoids Cursor reconnect loops).
    let stream = tokio_stream::once(Ok::<_, Infallible>(
        Event::default().comment("connected"),
    ))
    .chain(tokio_stream::pending());
    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        )
        .into_response()
}

/// Inputs: shared app state and shutdown token. Outputs: HTTP router for MCP traffic.
fn gateway_router(app: Arc<AppInner>, child: CancellationToken) -> Router {
    let gateway = Gateway { app: app.clone() };
    let service = StreamableHttpService::new(
        {
            let gateway = gateway.clone();
            move || Ok(gateway.clone())
        },
        LocalSessionManager::default().into(),
        // Stateless JSON POSTs; soft_get_sse upgrades GET 405 → 200 keep-alive SSE.
        StreamableHttpServerConfig::default()
            .with_cancellation_token(child)
            .with_allowed_hosts(["127.0.0.1", "localhost", "127.0.0.1:7341", "localhost:7341"])
            .with_stateful_mode(false)
            .with_json_response(true),
    );

    Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn(soft_get_sse))
        .layer(middleware::from_fn_with_state(app, auth_middleware))
}

/// Inputs: bearer token. Outputs: true when Funnelit on BIND_ADDR answers initialize.
pub async fn existing_endpoint_healthy(token: &str) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build();
    let Ok(client) = client else {
        return false;
    };
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": { "name": "funnelit-health", "version": "0" }
        }
    });
    let Ok(resp) = client
        .post(ENDPOINT_URL)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header(header::CONTENT_TYPE, "application/json")
        .json(&body)
        .send()
        .await
    else {
        return false;
    };
    if !resp.status().is_success() {
        return false;
    }
    let Ok(v) = resp.json::<serde_json::Value>().await else {
        return false;
    };
    if v.get("error").is_some() {
        return false;
    }
    v.get("result")
        .and_then(|r| r.get("serverInfo"))
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        == Some("funnelit")
}

/// Inputs: shared app state. Outputs: Ok(()) when the funnel HTTP server is listening.
pub async fn start(app: Arc<AppInner>) -> Result<(), String> {
    let _guard = app.lifecycle.lock().await;
    if app.running.load(Ordering::SeqCst) {
        return Ok(());
    }
    let ct = CancellationToken::new();
    let router = gateway_router(app.clone(), ct.child_token());

    let listener = match TcpListener::bind(BIND_ADDR).await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            let token = app.token.read().await.clone();
            // External process owns the port — do not mark local `running`.
            if existing_endpoint_healthy(&token).await {
                return Ok(());
            }
            return Err(format!(
                "{BIND_ADDR} already in use (quit the other funnelit, then Resume)"
            ));
        }
        Err(err) => return Err(err.to_string()),
    };
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
    let _guard = app.lifecycle.lock().await;
    if let Some(ct) = app.shutdown.write().await.take() {
        ct.cancel();
    }
    if let Some(mut task) = app.server_task.write().await.take() {
        match tokio::time::timeout(std::time::Duration::from_secs(3), &mut task).await {
            Ok(_) => {}
            Err(_) => {
                task.abort();
                let _ = task.await;
            }
        }
    }
    app.pool.clear().await;
    app.running.store(false, Ordering::SeqCst);
    Ok(())
}

/// Inputs: none. Outputs: funnel public URL constant.
pub fn endpoint_url() -> &'static str {
    ENDPOINT_URL
}

/// Inputs: shared app state. Outputs: Ok(()) after stdio MCP session ends.
pub async fn serve_stdio(app: Arc<AppInner>) -> Result<(), String> {
    let gateway = Gateway { app };
    let running = gateway
        .serve(stdio())
        .await
        .map_err(|e| e.to_string())?;
    running.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use tower::ServiceExt;

    const TEST_TOKEN: &str = "test-token";
    const PROTOCOL_VERSION: &str = "2025-06-18";

    #[test]
    fn bearer_matching_is_exact() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer secret-token".parse().unwrap(),
        );
        assert!(bearer_ok(&headers, "secret-token"));
        assert!(!bearer_ok(&headers, "other"));
    }

    /// Regression: Vec outputSchema is type "array" and panics tool_router at tools/list.
    #[test]
    fn tool_router_list_all_does_not_panic() {
        let tools = Gateway::list_gateway_tools();
        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"list_mcps"));
        assert!(names.contains(&"list_mcp_tools"));
        assert!(names.contains(&"execute_mcp_tool"));
    }

    #[test]
    fn cursor_safe_wraps_array_structured_content() {
        let raw = CallToolResult::structured(serde_json::json!([1, 2]));
        let safe = cursor_safe_tool_result(raw);
        assert!(safe.structured_content.as_ref().unwrap().is_object());
        assert_eq!(
            safe.structured_content.unwrap()["result"],
            serde_json::json!([1, 2])
        );
        let obj = CallToolResult::structured(serde_json::json!({ "ok": true }));
        let kept = cursor_safe_tool_result(obj);
        assert_eq!(kept.structured_content.unwrap()["ok"], true);
    }

    /// Inputs: app config. Outputs: app state with a fixed bearer token.
    fn test_app(config: AppConfig) -> Arc<AppInner> {
        Arc::new(AppInner {
            dir: tempfile::tempdir().unwrap().keep(),
            config: Arc::new(RwLock::new(config)),
            pool: Arc::new(crate::pool::ClientPool::new()),
            token: Arc::new(RwLock::new(TEST_TOKEN.into())),
            running: Arc::new(AtomicBool::new(false)),
            exiting: Arc::new(AtomicBool::new(false)),
            lifecycle: Arc::new(Mutex::new(())),
            shutdown: Arc::new(RwLock::new(None)),
            server_task: Arc::new(RwLock::new(None)),
        })
    }

    /// Inputs: router, session, request id, method, params. Outputs: status, headers, JSON body.
    async fn post_rpc(
        router: &Router,
        session: Option<&str>,
        id: Option<u64>,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> (StatusCode, HeaderMap, serde_json::Value) {
        let mut body = serde_json::json!({ "jsonrpc": "2.0", "method": method });
        if let Some(id) = id {
            body["id"] = id.into();
        }
        if let Some(params) = params {
            body["params"] = params;
        }
        let mut builder = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header(header::HOST, "127.0.0.1:7341")
            .header(header::AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
            .header(header::ACCEPT, "application/json, text/event-stream")
            .header(header::CONTENT_TYPE, "application/json")
            .header("MCP-Protocol-Version", PROTOCOL_VERSION);
        if let Some(session) = session {
            builder = builder.header("Mcp-Session-Id", session);
        }
        let response = router
            .clone()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = if body.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&body).unwrap_or_else(|e| {
                panic!("status {status}: {e}: {}", String::from_utf8_lossy(&body))
            })
        };
        (status, headers, json)
    }

    /// Inputs: router and optional session. Outputs: HTTP status for GET /mcp.
    async fn get_mcp_status(router: &Router, session: Option<&str>) -> StatusCode {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/mcp")
            .header(header::HOST, "127.0.0.1:7341")
            .header(header::AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
            .header(header::ACCEPT, "text/event-stream")
            .header("MCP-Protocol-Version", PROTOCOL_VERSION);
        if let Some(session) = session {
            builder = builder.header("Mcp-Session-Id", session);
        }
        router
            .clone()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn tools_list_returns_gateway_tools_over_http() {
        let app = test_app(AppConfig::default());
        let ct = CancellationToken::new();
        let router = gateway_router(app, ct.child_token());

        let initialize = post_rpc(
            &router,
            None,
            Some(1),
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "funnelit-test", "version": "0" }
            })),
        )
        .await;
        assert_eq!(initialize.0, StatusCode::OK);
        assert_eq!(
            initialize
                .1
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );

        let initialized =
            post_rpc(&router, None, None, "notifications/initialized", None).await;
        assert_eq!(initialized.0, StatusCode::ACCEPTED);

        // Soft GET returns 200 SSE keep-alive.
        assert_eq!(get_mcp_status(&router, None).await, StatusCode::OK);

        let started = std::time::Instant::now();
        let list = post_rpc(&router, None, Some(2), "tools/list", None).await;
        assert_eq!(list.0, StatusCode::OK);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert_eq!(
            list.1
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
        let names: Vec<_> = list.2["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["execute_mcp_tool", "list_mcp_tools", "list_mcps"]);

        let ping = post_rpc(&router, None, Some(3), "ping", None).await;
        assert_eq!(ping.0, StatusCode::OK);

        let call = post_rpc(
            &router,
            None,
            Some(4),
            "tools/call",
            Some(serde_json::json!({ "name": "list_mcps", "arguments": {} })),
        )
        .await;
        assert_eq!(call.0, StatusCode::OK);
        assert_ne!(call.2["result"]["isError"], true);

        ct.cancel();
    }
}
