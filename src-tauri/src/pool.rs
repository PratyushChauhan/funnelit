//! Lazy, reusable upstream MCP client cache.

use std::{collections::HashMap, sync::Arc, time::Duration};

use http::{HeaderName, HeaderValue};
use rmcp::{
    model::{CallToolRequestParams, CallToolResult, Tool},
    service::{Peer, RunningService},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, ConfigureCommandExt,
        StreamableHttpClientTransport, TokioChildProcess,
    },
    RoleClient, ServiceExt,
};
use tokio::{process::Command, sync::Mutex};

use crate::{
    config::{
        self, get_bearer_secret, get_env_secret, get_header_secret, redact, transport_fingerprint,
        ConfigError, McpServer, McpTransport,
    },
};

struct Cached {
    peer: Peer<RoleClient>,
    running: RunningService<RoleClient, ()>,
    fingerprint: String,
}

type Slot = Arc<Mutex<Option<Cached>>>;

#[derive(Default)]
pub struct ClientPool {
    slots: Mutex<HashMap<String, Slot>>,
}

impl ClientPool {
    /// Inputs: none. Outputs: empty pool.
    pub fn new() -> Self {
        Self::default()
    }

    async fn slot(&self, id: &str) -> Slot {
        let mut slots = self.slots.lock().await;
        slots
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(None)))
            .clone()
    }

    /// Inputs: mcp id. Outputs: unit after dropping any cached client.
    pub async fn invalidate(&self, id: &str) {
        let slot = {
            let mut slots = self.slots.lock().await;
            slots.remove(id)
        };
        if let Some(slot) = slot {
            if let Some(mut cached) = slot.lock().await.take() {
                let _ = cached
                    .running
                    .close_with_timeout(Duration::from_secs(2))
                    .await;
            }
        }
    }

    /// Inputs: none. Outputs: unit after closing every cached client.
    pub async fn clear(&self) {
        let slots = {
            let mut guard = self.slots.lock().await;
            std::mem::take(&mut *guard)
        };
        for (_, slot) in slots {
            if let Some(mut cached) = slot.lock().await.take() {
                let _ = cached
                    .running
                    .close_with_timeout(Duration::from_secs(2))
                    .await;
            }
        }
    }

    /// Inputs: server definition. Outputs: cloned peer for the live upstream.
    pub async fn peer(&self, server: &McpServer) -> Result<Peer<RoleClient>, ConfigError> {
        if !server.enabled {
            return Err(ConfigError::msg("mcp is disabled"));
        }
        let fp = transport_fingerprint(server);
        let slot = self.slot(&server.id).await;
        let mut guard = slot.lock().await;
        if let Some(cached) = guard.as_ref() {
            if cached.fingerprint == fp && !cached.peer.is_transport_closed() {
                return Ok(cached.peer.clone());
            }
        }
        if let Some(mut old) = guard.take() {
            drop(guard);
            let _ = old.running.close_with_timeout(Duration::from_secs(2)).await;
            guard = slot.lock().await;
            if let Some(cached) = guard.as_ref() {
                if cached.fingerprint == fp && !cached.peer.is_transport_closed() {
                    return Ok(cached.peer.clone());
                }
            }
        }
        let running = connect(server).await?;
        let peer = running.peer().clone();
        *guard = Some(Cached {
            peer: peer.clone(),
            running,
            fingerprint: fp,
        });
        Ok(peer)
    }

    /// Inputs: server. Outputs: upstream tool list (reconnects once on closed transport).
    pub async fn list_tools(&self, server: &McpServer) -> Result<Vec<Tool>, ConfigError> {
        match self.peer(server).await?.list_all_tools().await {
            Ok(tools) => {
                Ok(tools)
            }
            Err(err) => {
                self.invalidate(&server.id).await;
                self.peer(server)
                    .await?
                    .list_all_tools()
                    .await
                    .map_err(|e| ConfigError::msg(redact(&format!("{err}; retry: {e}"))))
            }
        }
    }

    /// Inputs: server, tool name, arguments. Outputs: upstream CallToolResult (no auto-retry).
    pub async fn call_tool(
        &self,
        server: &McpServer,
        tool_name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, ConfigError> {
        let peer = self.peer(server).await?;
        let mut params = CallToolRequestParams::new(tool_name.to_string());
        if let Some(args) = arguments {
            params = params.with_arguments(args);
        }
        let result = peer
            .call_tool(params)
            .await
            .map_err(|e| ConfigError::msg(redact(&e.to_string())));
        result
    }
}

/// Inputs: server definition. Outputs: connected RunningService.
async fn connect(server: &McpServer) -> Result<RunningService<RoleClient, ()>, ConfigError> {
    match &server.transport {
        McpTransport::Stdio {
            command,
            args,
            env_keys,
        } => {
            let cmd = Command::new(command).configure(|c| {
                c.args(args);
                c.env_clear();
                if let Ok(path) = std::env::var("PATH") {
                    c.env("PATH", path);
                }
                if let Ok(home) = std::env::var("HOME") {
                    c.env("HOME", home);
                }
                for key in env_keys {
                    if let Some(val) = get_env_secret(&server.id, key) {
                        c.env(key, val);
                    }
                }
            });
            let transport = TokioChildProcess::new(cmd)
                .map_err(|e| ConfigError::msg(redact(&e.to_string())))?;
            ().serve(transport)
                .await
                .map_err(|e| ConfigError::msg(redact(&e.to_string())))
        }
        McpTransport::Http {
            url,
            header_keys,
            has_bearer,
        } => {
            config::validate_http_url(url)?;
            let mut headers = HashMap::new();
            for key in header_keys {
                if let Some(val) = get_header_secret(&server.id, key) {
                    let name = HeaderName::from_bytes(key.as_bytes())
                        .map_err(|e| ConfigError::msg(e.to_string()))?;
                    let value =
                        HeaderValue::from_str(&val).map_err(|e| ConfigError::msg(e.to_string()))?;
                    headers.insert(name, value);
                }
            }
            let mut cfg = StreamableHttpClientTransportConfig::with_uri(url.clone())
                .custom_headers(headers)
                .reinit_on_expired_session(true);
            if *has_bearer {
                if let Some(token) = get_bearer_secret(&server.id) {
                    cfg = cfg.auth_header(token);
                }
            }
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(60))
                .build()
                .map_err(|e| ConfigError::msg(e.to_string()))?;
            let transport = StreamableHttpClientTransport::with_client(client, cfg);
            ().serve(transport)
                .await
                .map_err(|e| ConfigError::msg(redact(&e.to_string())))
        }
    }
}

/// Shared pool handle used by gateway and UI commands.
pub type SharedPool = Arc<ClientPool>;
