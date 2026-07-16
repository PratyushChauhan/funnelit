//! Persisted MCP definitions and OS-keychain secrets.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use url::Url;

const SERVICE: &str = "funnelit";
const TOKEN_USER: &str = "endpoint-token";
pub const ENDPOINT_URL: &str = "http://127.0.0.1:7341/mcp";
pub const BIND_ADDR: &str = "127.0.0.1:7341";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        /// Environment variable names; values live in the keychain.
        #[serde(default)]
        env_keys: Vec<String>,
    },
    Http {
        url: String,
        /// Header names; values live in the keychain.
        #[serde(default)]
        header_keys: Vec<String>,
        /// Optional bearer token stored in the keychain when true.
        #[serde(default)]
        has_bearer: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub transport: McpTransport,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub servers: Vec<McpServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMap {
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub bearer: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Keyring(#[from] keyring::Error),
}

impl ConfigError {
    /// Inputs: message. Outputs: typed config error.
    pub fn msg(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

/// Inputs: config directory path. Outputs: path to servers.json.
pub fn config_file(dir: &Path) -> PathBuf {
    dir.join("servers.json")
}

/// Inputs: config directory. Outputs: loaded config (empty if missing).
pub fn load_config(dir: &Path) -> Result<AppConfig, ConfigError> {
    let path = config_file(dir);
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

/// Inputs: config directory and config. Outputs: unit on success.
pub fn save_config(dir: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    fs::create_dir_all(dir)?;
    fs::write(config_file(dir), serde_json::to_string_pretty(config)?)?;
    Ok(())
}

/// Inputs: none. Outputs: new high-entropy bearer token.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Inputs: none. Outputs: existing or newly generated endpoint token.
pub fn ensure_endpoint_token() -> Result<String, ConfigError> {
    let entry = keyring::Entry::new(SERVICE, TOKEN_USER)?;
    match entry.get_password() {
        Ok(token) if !token.is_empty() => Ok(token),
        _ => {
            let token = generate_token();
            entry.set_password(&token)?;
            Ok(token)
        }
    }
}

/// Inputs: none. Outputs: regenerated endpoint token.
pub fn rotate_endpoint_token() -> Result<String, ConfigError> {
    let token = generate_token();
    keyring::Entry::new(SERVICE, TOKEN_USER)?.set_password(&token)?;
    Ok(token)
}

fn secret_user(kind: &str, mcp_id: &str, key: &str) -> String {
    format!("{kind}:{mcp_id}:{key}")
}

/// Inputs: mcp id, env/header/bearer secrets. Outputs: unit on success.
pub fn store_secrets(mcp_id: &str, secrets: &SecretMap) -> Result<(), ConfigError> {
    for (k, v) in &secrets.env {
        if v.is_empty() {
            continue;
        }
        keyring::Entry::new(SERVICE, &secret_user("env", mcp_id, k))?.set_password(v)?;
    }
    for (k, v) in &secrets.headers {
        if v.is_empty() {
            continue;
        }
        keyring::Entry::new(SERVICE, &secret_user("hdr", mcp_id, k))?.set_password(v)?;
    }
    if let Some(bearer) = &secrets.bearer {
        if !bearer.is_empty() {
            keyring::Entry::new(SERVICE, &secret_user("hdr", mcp_id, "authorization_bearer"))?
                .set_password(bearer)?;
        }
    }
    Ok(())
}

/// Inputs: mcp id and key names. Outputs: unit; missing secrets ignored.
pub fn delete_secrets(mcp_id: &str, env_keys: &[String], header_keys: &[String], has_bearer: bool) {
    for k in env_keys {
        let _ = keyring::Entry::new(SERVICE, &secret_user("env", mcp_id, k))
            .and_then(|e| e.delete_credential());
    }
    for k in header_keys {
        let _ = keyring::Entry::new(SERVICE, &secret_user("hdr", mcp_id, k))
            .and_then(|e| e.delete_credential());
    }
    if has_bearer {
        let _ = keyring::Entry::new(SERVICE, &secret_user("hdr", mcp_id, "authorization_bearer"))
            .and_then(|e| e.delete_credential());
    }
}

/// Inputs: mcp id and env key. Outputs: secret value if present.
pub fn get_env_secret(mcp_id: &str, key: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, &secret_user("env", mcp_id, key))
        .ok()
        .and_then(|e| e.get_password().ok())
}

/// Inputs: mcp id and header key. Outputs: secret value if present.
pub fn get_header_secret(mcp_id: &str, key: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, &secret_user("hdr", mcp_id, key))
        .ok()
        .and_then(|e| e.get_password().ok())
}

/// Inputs: mcp id. Outputs: optional bearer token for upstream HTTP.
pub fn get_bearer_secret(mcp_id: &str) -> Option<String> {
    get_header_secret(mcp_id, "authorization_bearer")
}

/// Inputs: candidate MCP record. Outputs: Ok(()) or validation error.
pub fn validate_server(server: &McpServer) -> Result<(), ConfigError> {
    if server.name.trim().is_empty() {
        return Err(ConfigError::msg("name is required"));
    }
    if server.id.trim().is_empty() {
        return Err(ConfigError::msg("id is required"));
    }
    match &server.transport {
        McpTransport::Stdio { command, .. } => {
            if command.trim().is_empty() {
                return Err(ConfigError::msg("command is required"));
            }
            if command.contains([' ', '\t', '\n', ';', '|', '&', '`', '$']) {
                return Err(ConfigError::msg(
                    "command must be an executable path, not a shell string",
                ));
            }
        }
        McpTransport::Http { url, .. } => validate_http_url(url)?,
    }
    Ok(())
}

/// Inputs: URL string. Outputs: Ok(()) when the URL is an allowed MCP endpoint.
pub fn validate_http_url(raw: &str) -> Result<(), ConfigError> {
    let parsed = Url::parse(raw).map_err(|e| ConfigError::msg(format!("invalid url: {e}")))?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" => {
            let host = parsed.host_str().unwrap_or_default();
            if matches!(host, "127.0.0.1" | "localhost" | "::1") {
                Ok(())
            } else {
                Err(ConfigError::msg(
                    "plain HTTP is only allowed for loopback hosts",
                ))
            }
        }
        _ => Err(ConfigError::msg("url must be http or https")),
    }
}

/// Inputs: server transport. Outputs: fingerprint used to invalidate caches.
pub fn transport_fingerprint(server: &McpServer) -> String {
    serde_json::to_string(&server.transport).unwrap_or_default()
}

/// Inputs: error text. Outputs: redacted text without common secret shapes.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for needle in ["Bearer ", "authorization=", "api_key=", "API_KEY="] {
        if let Some(idx) = out.find(needle) {
            let end = (idx + needle.len() + 12).min(out.len());
            out.replace_range(idx..end, &format!("{needle}***"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_shell_stdio_and_remote_http() {
        let bad_stdio = McpServer {
            id: "a".into(),
            name: "a".into(),
            enabled: true,
            transport: McpTransport::Stdio {
                command: "npx -y x".into(),
                args: vec![],
                env_keys: vec![],
            },
        };
        assert!(validate_server(&bad_stdio).is_err());
        assert!(validate_http_url("http://example.com/mcp").is_err());
        assert!(validate_http_url("http://127.0.0.1:9/mcp").is_ok());
        assert!(validate_http_url("https://example.com/mcp").is_ok());
    }

    #[test]
    fn redacts_bearer_prefix() {
        assert!(redact("Authorization: Bearer abcdefghijklmnop").contains("***"));
    }
}
