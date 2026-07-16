//! Persisted MCP definitions and OS-keychain secrets.

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::Write,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

/// Inputs: config directory and config. Outputs: unit after atomic write.
pub fn save_config(dir: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    fs::create_dir_all(dir)?;
    let path = config_file(dir);
    let tmp = dir.join(format!("servers.json.tmp.{}", std::process::id()));
    let data = serde_json::to_string_pretty(config)?;
    {
        let mut file = File::create(&tmp)?;
        file.write_all(data.as_bytes())?;
        file.sync_all()?;
    }
    replace_file(&tmp, &path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Inputs: temp path and destination. Outputs: unit after replacing destination.
fn replace_file(tmp: &Path, path: &Path) -> Result<(), std::io::Error> {
    #[cfg(windows)]
    {
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    fs::rename(tmp, path)
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
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let token = generate_token();
            entry.set_password(&token)?;
            Ok(token)
        }
        Err(err) => Err(err.into()),
    }
}

/// Inputs: none. Outputs: regenerated endpoint token.
pub fn rotate_endpoint_token() -> Result<String, ConfigError> {
    let token = generate_token();
    keyring::Entry::new(SERVICE, TOKEN_USER)?.set_password(&token)?;
    Ok(token)
}

fn encode_part(value: &str) -> String {
    value.replace('%', "%25").replace(':', "%3A")
}

fn secret_user(kind: &str, mcp_id: &str, key: &str) -> String {
    format!("{kind}:{}:{}", encode_part(mcp_id), encode_part(key))
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

fn delete_secret_entry(user: &str) -> Result<(), ConfigError> {
    match keyring::Entry::new(SERVICE, user).and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Inputs: mcp id and key names. Outputs: Ok(()) or first non-NoEntry keyring error.
pub fn delete_secrets(
    mcp_id: &str,
    env_keys: &[String],
    header_keys: &[String],
    has_bearer: bool,
) -> Result<(), ConfigError> {
    for k in env_keys {
        delete_secret_entry(&secret_user("env", mcp_id, k))?;
    }
    for k in header_keys {
        delete_secret_entry(&secret_user("hdr", mcp_id, k))?;
    }
    if has_bearer {
        delete_secret_entry(&secret_user("hdr", mcp_id, "authorization_bearer"))?;
    }
    Ok(())
}

/// Inputs: previous and next server records. Outputs: Ok after deleting removed secret keys.
pub fn prune_secrets(previous: &McpServer, next: &McpServer) -> Result<(), ConfigError> {
    let (old_env, old_hdr, old_bearer) = secret_keysets(previous);
    let (new_env, new_hdr, new_bearer) = secret_keysets(next);
    let drop_env: Vec<_> = old_env.difference(&new_env).cloned().collect();
    let drop_hdr: Vec<_> = old_hdr.difference(&new_hdr).cloned().collect();
    delete_secrets(
        &previous.id,
        &drop_env,
        &drop_hdr,
        old_bearer && !new_bearer,
    )
}

fn secret_keysets(server: &McpServer) -> (HashSet<String>, HashSet<String>, bool) {
    match &server.transport {
        McpTransport::Stdio { env_keys, .. } => {
            (env_keys.iter().cloned().collect(), HashSet::new(), false)
        }
        McpTransport::Http {
            header_keys,
            has_bearer,
            ..
        } => (
            HashSet::new(),
            header_keys.iter().cloned().collect(),
            *has_bearer,
        ),
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

fn set_secret(user: &str, value: &str) -> Result<(), ConfigError> {
    keyring::Entry::new(SERVICE, user)?.set_password(value)?;
    Ok(())
}

fn get_secret(user: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, user)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.is_empty())
}

/// Inputs: mcp id, client id, optional secret. Outputs: unit after keychain write.
pub fn store_oauth_client(
    mcp_id: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<(), ConfigError> {
    set_secret(&secret_user("oauth", mcp_id, "client_id"), client_id)?;
    match client_secret.filter(|s| !s.is_empty()) {
        Some(secret) => set_secret(&secret_user("oauth", mcp_id, "client_secret"), secret)?,
        None => delete_secret_entry(&secret_user("oauth", mcp_id, "client_secret"))?,
    }
    Ok(())
}

/// Inputs: mcp id. Outputs: saved OAuth client id if present.
pub fn get_oauth_client_id(mcp_id: &str) -> Option<String> {
    get_secret(&secret_user("oauth", mcp_id, "client_id"))
}

/// Inputs: mcp id. Outputs: saved OAuth client secret if present.
pub fn get_oauth_client_secret(mcp_id: &str) -> Option<String> {
    get_secret(&secret_user("oauth", mcp_id, "client_secret"))
}

/// Inputs: host, client id, optional secret. Outputs: unit after keychain write.
pub fn store_oauth_host_client(
    host: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<(), ConfigError> {
    set_secret(&secret_user("oauthhost", host, "client_id"), client_id)?;
    match client_secret.filter(|s| !s.is_empty()) {
        Some(secret) => set_secret(&secret_user("oauthhost", host, "client_secret"), secret)?,
        None => delete_secret_entry(&secret_user("oauthhost", host, "client_secret"))?,
    }
    Ok(())
}

/// Inputs: host. Outputs: host-scoped OAuth client id if present.
pub fn get_oauth_host_client_id(host: &str) -> Option<String> {
    get_secret(&secret_user("oauthhost", host, "client_id"))
}

/// Inputs: host. Outputs: host-scoped OAuth client secret if present.
pub fn get_oauth_host_client_secret(host: &str) -> Option<String> {
    get_secret(&secret_user("oauthhost", host, "client_secret"))
}

/// Inputs: mcp id and refresh token. Outputs: unit after keychain write.
pub fn store_oauth_refresh(mcp_id: &str, refresh_token: &str) -> Result<(), ConfigError> {
    set_secret(
        &secret_user("oauth", mcp_id, "refresh_token"),
        refresh_token,
    )
}

/// Inputs: mcp id. Outputs: refresh token if present.
pub fn get_oauth_refresh(mcp_id: &str) -> Option<String> {
    get_secret(&secret_user("oauth", mcp_id, "refresh_token"))
}

/// Inputs: mcp id. Outputs: true when access or refresh credentials exist.
pub fn oauth_session_present(mcp_id: &str) -> bool {
    get_bearer_secret(mcp_id).is_some() || get_oauth_refresh(mcp_id).is_some()
}

/// Inputs: mcp id. Outputs: unit after deleting OAuth secrets for that MCP.
pub fn delete_oauth_secrets(mcp_id: &str) -> Result<(), ConfigError> {
    for key in ["client_id", "client_secret", "refresh_token"] {
        delete_secret_entry(&secret_user("oauth", mcp_id, key))?;
    }
    Ok(())
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

/// Inputs: config and candidate id that must be unique among other servers. Outputs: Ok or duplicate error.
pub fn validate_unique_id(
    config: &AppConfig,
    id: &str,
    replacing: bool,
) -> Result<(), ConfigError> {
    let count = config.servers.iter().filter(|s| s.id == id).count();
    let allowed = if replacing { 1 } else { 0 };
    if count > allowed {
        return Err(ConfigError::msg("duplicate mcp id"));
    }
    Ok(())
}

/// Inputs: URL string. Outputs: Ok(()) when the URL is an allowed MCP endpoint.
pub fn validate_http_url(raw: &str) -> Result<(), ConfigError> {
    let parsed = Url::parse(raw).map_err(|e| ConfigError::msg(format!("invalid url: {e}")))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(ConfigError::msg("url must not contain credentials"));
    }
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

/// Inputs: error text. Outputs: redacted text with secret values fully masked.
pub fn redact(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = text.to_string();
    let mut shift = 0isize;
    for needle in ["bearer ", "authorization=", "api_key="] {
        let mut search_from = 0usize;
        while let Some(rel) = lower[search_from..].find(needle) {
            let idx = search_from + rel;
            let value_start = idx + needle.len();
            let value_end = text[value_start..]
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | '&'))
                .map(|i| value_start + i)
                .unwrap_or(text.len());
            let adj_start = (value_start as isize + shift) as usize;
            let adj_end = (value_end as isize + shift) as usize;
            let old_len = adj_end - adj_start;
            out.replace_range(adj_start..adj_end, "***");
            shift += 3 - old_len as isize;
            search_from = value_end;
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
        assert!(validate_http_url("https://user:pass@example.com/mcp").is_err());
    }

    #[test]
    fn redacts_full_bearer_tokens() {
        let token = "abcdefghijklmnopqr";
        let redacted = redact(&format!("Authorization: Bearer {token} and BEARER {token}"));
        assert!(!redacted.contains(token));
        assert!(redacted.to_ascii_lowercase().contains("bearer ***"));
    }

    #[test]
    fn save_config_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = AppConfig::default();
        save_config(dir.path(), &cfg).unwrap();
        cfg.servers.push(McpServer {
            id: "x".into(),
            name: "x".into(),
            enabled: true,
            transport: McpTransport::Stdio {
                command: "/bin/true".into(),
                args: vec![],
                env_keys: vec![],
            },
        });
        save_config(dir.path(), &cfg).unwrap();
        let loaded = load_config(dir.path()).unwrap();
        assert_eq!(loaded.servers.len(), 1);
        assert_eq!(loaded.servers[0].id, "x");
    }

    #[test]
    fn secret_user_encodes_colons() {
        assert_eq!(secret_user("env", "a:b", "c:d"), "env:a%3Ab:c%3Ad");
    }

    #[test]
    fn oauth_secret_users_are_namespaced() {
        assert_eq!(
            secret_user("oauth", "mcp-1", "client_secret"),
            "oauth:mcp-1:client_secret"
        );
        assert_eq!(
            secret_user("oauthhost", "mcp.slack.com", "client_id"),
            "oauthhost:mcp.slack.com:client_id"
        );
        assert_eq!(
            secret_user("oauth", "mcp-1", "refresh_token"),
            "oauth:mcp-1:refresh_token"
        );
    }

    #[test]
    fn rejects_duplicate_ids() {
        let cfg = AppConfig {
            servers: vec![McpServer {
                id: "x".into(),
                name: "x".into(),
                enabled: true,
                transport: McpTransport::Stdio {
                    command: "/bin/true".into(),
                    args: vec![],
                    env_keys: vec![],
                },
            }],
        };
        assert!(validate_unique_id(&cfg, "x", false).is_err());
        assert!(validate_unique_id(&cfg, "x", true).is_ok());
    }
}
