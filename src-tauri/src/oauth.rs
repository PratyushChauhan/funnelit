//! Browser OAuth for HTTP MCP servers (discovery, PKCE, DCR, loopback callback).

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::Duration,
};

use axum::{
    extract::{Query, State as AxumState},
    response::Html,
    routing::get,
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{oneshot, Mutex};
use url::Url;

use crate::config::{self, redact, SecretMap};

pub const REDIRECT_URI: &str = "http://127.0.0.1:7342/oauth/callback";
const BIND_ADDR: &str = "127.0.0.1:7342";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

/// Inputs: none. Outputs: process-wide mutex serializing browser OAuth flows.
fn oauth_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthProbe {
    pub oauth: bool,
    pub label: String,
    pub authorization_server: Option<String>,
    pub scopes: Vec<String>,
    /// True when the AS supports Dynamic Client Registration.
    pub supports_dcr: bool,
    pub needs_client_id: bool,
    pub has_saved_client: bool,
    pub connected: bool,
}

struct AbortOnDrop(Option<tokio::task::JoinHandle<()>>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

#[derive(Debug, Deserialize)]
struct ProtectedResource {
    #[serde(default)]
    authorization_servers: Vec<String>,
    #[serde(default)]
    scopes_supported: Vec<String>,
    #[serde(default)]
    resource_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthServerMeta {
    authorization_endpoint: String,
    token_endpoint: String,
    #[serde(default)]
    registration_endpoint: Option<String>,
    #[serde(default)]
    scopes_supported: Vec<String>,
    #[serde(default)]
    code_challenge_methods_supported: Vec<String>,
    #[serde(default)]
    token_endpoint_auth_methods_supported: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RegistrationResponse {
    client_id: String,
    #[serde(default)]
    client_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
    #[serde(default)]
    authed_user: Option<SlackAuthedUser>,
}

#[derive(Debug, Deserialize)]
struct SlackAuthedUser {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Clone)]
struct CallbackState {
    expected_state: String,
    tx: Arc<std::sync::Mutex<Option<oneshot::Sender<Result<(String, String), String>>>>>,
}

/// Inputs: MCP URL and optional mcp id for saved client lookup. Outputs: OAuth probe result.
pub async fn probe(url: &str, mcp_id: Option<&str>) -> Result<AuthProbe, String> {
    config::validate_http_url(url).map_err(|e| e.to_string())?;
    let connected = mcp_id
        .filter(|id| !id.is_empty())
        .is_some_and(config::oauth_session_present);
    let Some((resource, as_meta)) = discover(url).await? else {
        return Ok(AuthProbe {
            oauth: false,
            label: "Sign in".into(),
            authorization_server: None,
            scopes: vec![],
            supports_dcr: false,
            needs_client_id: false,
            has_saved_client: false,
            connected,
        });
    };
    let label = sign_in_label(url, resource.resource_name.as_deref());
    let supports_dcr = as_meta.registration_endpoint.is_some();
    let has_saved = mcp_id
        .filter(|id| !id.is_empty())
        .and_then(|id| config::get_oauth_client_id(id))
        .is_some()
        || host_client_id(url).is_some();
    Ok(AuthProbe {
        oauth: true,
        label,
        authorization_server: resource.authorization_servers.first().cloned(),
        scopes: if resource.scopes_supported.is_empty() {
            as_meta.scopes_supported
        } else {
            resource.scopes_supported
        },
        supports_dcr,
        needs_client_id: !supports_dcr && !has_saved,
        has_saved_client: has_saved,
        connected,
    })
}

/// Inputs: MCP URL, mcp id, optional client credentials. Outputs: Ok after tokens are stored.
pub async fn authorize(
    url: &str,
    mcp_id: &str,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> Result<(), String> {
    config::validate_http_url(url).map_err(|e| e.to_string())?;
    if mcp_id.trim().is_empty() {
        return Err("mcp id is required".into());
    }
    let _guard = oauth_mutex().lock().await;

    let (resource, as_meta) = discover(url)
        .await?
        .ok_or_else(|| "this MCP does not advertise OAuth".to_string())?;
    if !as_meta.code_challenge_methods_supported.is_empty()
        && !as_meta
            .code_challenge_methods_supported
            .iter()
            .any(|m| m.eq_ignore_ascii_case("S256"))
    {
        return Err("authorization server does not support PKCE S256".into());
    }

    let scopes = if resource.scopes_supported.is_empty() {
        as_meta.scopes_supported.clone()
    } else {
        resource.scopes_supported.clone()
    };

    let (client_id, client_secret) = resolve_client(
        url,
        mcp_id,
        &as_meta,
        client_id.filter(|s| !s.trim().is_empty()),
        client_secret.filter(|s| !s.trim().is_empty()),
    )
    .await?;
    if client_secret_required(&as_meta.token_endpoint_auth_methods_supported)
        && client_secret.as_ref().is_none_or(|s| s.is_empty())
    {
        return Err("this MCP requires an OAuth client secret".into());
    }

    let verifier = pkce_verifier();
    let challenge = pkce_challenge(&verifier);
    let state = random_token(24);
    let auth_url = build_authorize_url(
        &as_meta.authorization_endpoint,
        &client_id,
        &scopes,
        &challenge,
        &state,
        url,
    )?;

    let (tx, rx) = oneshot::channel();
    let cb_state = CallbackState {
        expected_state: state.clone(),
        tx: Arc::new(std::sync::Mutex::new(Some(tx))),
    };
    let listener = tokio::net::TcpListener::bind(BIND_ADDR)
        .await
        .map_err(|e| format!("oauth callback bind failed: {e}"))?;
    let app = Router::new()
        .route("/oauth/callback", get(oauth_callback))
        .with_state(cb_state);
    let server = axum::serve(listener, app);
    let _abort = AbortOnDrop(Some(tokio::spawn(async move {
        let _ = server.await;
    })));

    open::that(&auth_url).map_err(|e| format!("failed to open browser: {e}"))?;

    let callback = tokio::time::timeout(CALLBACK_TIMEOUT, rx).await;
    drop(_abort);
    let (code, returned_state) = callback
        .map_err(|_| "sign-in timed out".to_string())?
        .map_err(|_| "sign-in cancelled".to_string())??;
    if returned_state != state {
        return Err("oauth state mismatch".into());
    }

    let tokens = exchange_code(
        &as_meta.token_endpoint,
        &client_id,
        client_secret.as_deref(),
        &code,
        &verifier,
        url,
    )
    .await?;

    config::store_oauth_client(mcp_id, &client_id, client_secret.as_deref())
        .map_err(|e| e.to_string())?;
    if let Some(host) = Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
    {
        let _ = config::store_oauth_host_client(&host, &client_id, client_secret.as_deref());
    }
    config::store_secrets(
        mcp_id,
        &SecretMap {
            bearer: Some(tokens.access_token),
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    if let Some(refresh) = tokens.refresh_token {
        config::store_oauth_refresh(mcp_id, &refresh).map_err(|e| e.to_string())?;
    }
    Ok(())
}

struct IssuedTokens {
    access_token: String,
    refresh_token: Option<String>,
}

/// Inputs: MCP URL. Outputs: protected-resource + AS metadata when OAuth is advertised.
async fn discover(url: &str) -> Result<Option<(ProtectedResource, AuthServerMeta)>, String> {
    let client = http_client()?;
    let candidates = discovery_urls(url)?;
    let mut resource: Option<ProtectedResource> = None;
    for candidate in candidates {
        if let Ok(resp) = client.get(&candidate).send().await {
            if resp.status().is_success() {
                if let Ok(body) = resp.json::<ProtectedResource>().await {
                    if !body.authorization_servers.is_empty() {
                        resource = Some(body);
                        break;
                    }
                }
            }
        }
    }
    let Some(resource) = resource else {
        return Ok(None);
    };
    let as_url = resource
        .authorization_servers
        .first()
        .ok_or_else(|| "missing authorization_servers".to_string())?;
    let meta_url = auth_server_metadata_url(as_url)?;
    let resp = client
        .get(&meta_url)
        .send()
        .await
        .map_err(|e| redact(&e.to_string()))?;
    if !resp.status().is_success() {
        return Err(format!(
            "authorization server metadata HTTP {}",
            resp.status()
        ));
    }
    let meta = resp
        .json::<AuthServerMeta>()
        .await
        .map_err(|e| redact(&e.to_string()))?;
    Ok(Some((resource, meta)))
}

/// Inputs: MCP URL. Outputs: candidate protected-resource metadata URLs.
fn discovery_urls(mcp_url: &str) -> Result<Vec<String>, String> {
    let parsed = Url::parse(mcp_url).map_err(|e| e.to_string())?;
    let origin = parsed.origin().ascii_serialization();
    let mut out = vec![format!("{origin}/.well-known/oauth-protected-resource")];
    let path = parsed.path().trim_matches('/');
    if !path.is_empty() {
        out.insert(
            0,
            format!("{origin}/.well-known/oauth-protected-resource/{path}"),
        );
    }
    if let Some(host) = parsed.host_str() {
        if host.contains("slack") {
            out.insert(
                0,
                "https://mcp.slack.com/.well-known/oauth-protected-resource".into(),
            );
        }
    }
    out.dedup();
    Ok(out)
}

/// Inputs: authorization server issuer URL. Outputs: metadata document URL.
fn auth_server_metadata_url(issuer: &str) -> Result<String, String> {
    let parsed = Url::parse(issuer).map_err(|e| e.to_string())?;
    let origin = parsed.origin().ascii_serialization();
    let path = parsed.path().trim_matches('/');
    if path.is_empty() {
        Ok(format!("{origin}/.well-known/oauth-authorization-server"))
    } else {
        Ok(format!(
            "{origin}/.well-known/oauth-authorization-server/{path}"
        ))
    }
}

/// Inputs: token endpoint auth methods. Outputs: true when a client secret is required.
fn client_secret_required(methods: &[String]) -> bool {
    if methods.is_empty() {
        return false;
    }
    let allows_none = methods.iter().any(|m| m == "none");
    let wants_secret = methods.iter().any(|m| m.contains("client_secret"));
    wants_secret && !allows_none
}

/// Inputs: supplied id, optional supplied secret, saved id/secret. Outputs: secret to use.
fn secret_for_client_id(
    id: &str,
    client_secret: Option<String>,
    saved_id: Option<String>,
    saved_secret: Option<String>,
) -> Option<String> {
    client_secret.or_else(|| {
        saved_id
            .filter(|saved| saved == id)
            .and_then(|_| saved_secret)
    })
}

/// Resolve OAuth client credentials for an MCP.
///
/// Inputs: MCP URL, id, AS metadata, optional credentials.
/// Outputs: client id and optional secret (BYO, saved, host-scoped, or DCR).
async fn resolve_client(
    url: &str,
    mcp_id: &str,
    as_meta: &AuthServerMeta,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> Result<(String, Option<String>), String> {
    if let Some(id) = client_id {
        let secret = secret_for_client_id(
            &id,
            client_secret,
            config::get_oauth_client_id(mcp_id),
            config::get_oauth_client_secret(mcp_id),
        );
        return Ok((id, secret));
    }
    if let Some(id) = config::get_oauth_client_id(mcp_id) {
        return Ok((
            id,
            client_secret.or_else(|| config::get_oauth_client_secret(mcp_id)),
        ));
    }
    if let Some(id) = host_client_id(url) {
        let host = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();
        return Ok((
            id,
            client_secret.or_else(|| config::get_oauth_host_client_secret(&host)),
        ));
    }
    if let Some(reg) = &as_meta.registration_endpoint {
        return register_client(reg).await;
    }
    Err("this server does not support automatic registration. \
         Add your OAuth Client ID under Advanced (bring your own app), \
         then Sign in again"
        .into())
}

/// Inputs: MCP URL. Outputs: host-scoped OAuth client id when previously saved.
fn host_client_id(url: &str) -> Option<String> {
    let host = Url::parse(url).ok()?.host_str()?.to_string();
    config::get_oauth_host_client_id(&host)
}

/// Inputs: DCR registration endpoint. Outputs: new client id and optional secret.
async fn register_client(endpoint: &str) -> Result<(String, Option<String>), String> {
    let client = http_client()?;
    let body = serde_json::json!({
        "client_name": "funnelit",
        "redirect_uris": [REDIRECT_URI],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
        "application_type": "native",
    });
    let resp = client
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| redact(&e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "client registration failed ({status}): {}",
            redact(&text)
        ));
    }
    let reg = resp
        .json::<RegistrationResponse>()
        .await
        .map_err(|e| redact(&e.to_string()))?;
    Ok((reg.client_id, reg.client_secret))
}

/// Build the browser authorization URL (PKCE S256 + resource indicator).
///
/// Inputs: authorize endpoint, client id, scopes, PKCE challenge, state, resource.
/// Outputs: browser authorization URL.
fn build_authorize_url(
    endpoint: &str,
    client_id: &str,
    scopes: &[String],
    challenge: &str,
    state: &str,
    resource: &str,
) -> Result<String, String> {
    let mut url = Url::parse(endpoint).map_err(|e| e.to_string())?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("client_id", client_id);
        q.append_pair("redirect_uri", REDIRECT_URI);
        q.append_pair("state", state);
        q.append_pair("code_challenge", challenge);
        q.append_pair("code_challenge_method", "S256");
        if !scopes.is_empty() {
            q.append_pair("scope", &scopes.join(" "));
        }
        q.append_pair("resource", resource);
    }
    Ok(url.to_string())
}

/// Inputs: token endpoint, client credentials, auth code, PKCE verifier, resource.
/// Outputs: access token and optional refresh token.
async fn exchange_code(
    token_endpoint: &str,
    client_id: &str,
    client_secret: Option<&str>,
    code: &str,
    verifier: &str,
    resource: &str,
) -> Result<IssuedTokens, String> {
    let client = http_client()?;
    let mut form = HashMap::new();
    form.insert("grant_type", "authorization_code".to_string());
    form.insert("code", code.to_string());
    form.insert("redirect_uri", REDIRECT_URI.to_string());
    form.insert("client_id", client_id.to_string());
    form.insert("code_verifier", verifier.to_string());
    form.insert("resource", resource.to_string());
    if let Some(secret) = client_secret {
        form.insert("client_secret", secret.to_string());
    }
    let resp = client
        .post(token_endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|e| redact(&e.to_string()))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| redact(&e.to_string()))?;
    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|e| redact(&format!("{e}: {body}")))?;
    if parsed.ok == Some(false) || parsed.error.is_some() {
        let detail = parsed
            .error_description
            .or(parsed.error)
            .unwrap_or_else(|| body.clone());
        return Err(format!("token exchange failed: {}", redact(&detail)));
    }
    if !status.is_success() && parsed.access_token.is_none() {
        return Err(format!("token exchange HTTP {status}: {}", redact(&body)));
    }
    let access = parsed
        .access_token
        .or_else(|| {
            parsed
                .authed_user
                .as_ref()
                .and_then(|u| u.access_token.clone())
        })
        .ok_or_else(|| "token response missing access_token".to_string())?;
    let refresh = parsed.refresh_token.or_else(|| {
        parsed
            .authed_user
            .as_ref()
            .and_then(|u| u.refresh_token.clone())
    });
    Ok(IssuedTokens {
        access_token: access,
        refresh_token: refresh,
    })
}

/// Inputs: callback state and query params. Outputs: HTML page; sends code via oneshot.
async fn oauth_callback(
    AxumState(state): AxumState<CallbackState>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<&'static str> {
    let result = if let Some(err) = params.get("error") {
        Err(format!(
            "provider error: {err} {}",
            params.get("error_description").cloned().unwrap_or_default()
        ))
    } else {
        match (params.get("code"), params.get("state")) {
            (Some(code), Some(returned)) if returned == &state.expected_state => {
                Ok((code.clone(), returned.clone()))
            }
            (Some(_), _) => Err("oauth state mismatch".into()),
            _ => Err("missing authorization code".into()),
        }
    };
    if let Some(tx) = state.tx.lock().ok().and_then(|mut g| g.take()) {
        let _ = tx.send(result);
    }
    Html(
        "<!doctype html><title>funnelit</title><body style='font-family:sans-serif;padding:2rem'>Signed in. You can close this window and return to funnelit.</body>",
    )
}

/// Inputs: MCP URL and optional resource name. Outputs: UI sign-in button label.
fn sign_in_label(url: &str, resource_name: Option<&str>) -> String {
    if let Ok(parsed) = Url::parse(url) {
        if parsed.host_str().is_some_and(|h| h.contains("slack")) {
            return "Sign in with Slack".into();
        }
    }
    if let Some(name) = resource_name.filter(|s| !s.is_empty()) {
        return format!("Sign in with {name}");
    }
    "Sign in".into()
}

/// Inputs: none. Outputs: high-entropy PKCE code verifier.
fn pkce_verifier() -> String {
    random_token(32)
}

/// Inputs: PKCE verifier. Outputs: S256 code challenge (base64url).
fn pkce_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

/// Inputs: byte length. Outputs: URL-safe base64 random token.
fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

/// Inputs: none. Outputs: reqwest client for OAuth HTTP calls.
fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_urls_include_path_aware() {
        let urls = discovery_urls("https://example.com/mcp").unwrap();
        assert!(urls
            .iter()
            .any(|u| u == "https://example.com/.well-known/oauth-protected-resource/mcp"));
        assert!(urls
            .iter()
            .any(|u| u == "https://example.com/.well-known/oauth-protected-resource"));
    }

    #[test]
    fn discovery_urls_prefer_slack_well_known() {
        let urls = discovery_urls("https://mcp.slack.com/mcp").unwrap();
        assert_eq!(
            urls[0],
            "https://mcp.slack.com/.well-known/oauth-protected-resource"
        );
    }

    #[test]
    fn auth_server_metadata_url_supports_path() {
        assert_eq!(
            auth_server_metadata_url("https://mcp.slack.com").unwrap(),
            "https://mcp.slack.com/.well-known/oauth-authorization-server"
        );
        assert_eq!(
            auth_server_metadata_url("https://auth.example.com/tenant").unwrap(),
            "https://auth.example.com/.well-known/oauth-authorization-server/tenant"
        );
    }

    #[test]
    fn pkce_challenge_is_s256_shaped() {
        let challenge = pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn slack_label() {
        assert_eq!(
            sign_in_label("https://mcp.slack.com/mcp", Some("Slack API")),
            "Sign in with Slack"
        );
        assert_eq!(
            sign_in_label("https://example.com/mcp", Some("Acme")),
            "Sign in with Acme"
        );
    }

    #[test]
    fn client_secret_required_from_methods() {
        assert!(!client_secret_required(&[]));
        assert!(!client_secret_required(&["none".into()]));
        assert!(!client_secret_required(&[
            "none".into(),
            "client_secret_post".into()
        ]));
        assert!(client_secret_required(&["client_secret_post".into()]));
    }

    #[test]
    fn secret_for_client_id_only_reuses_matching_saved_secret() {
        assert_eq!(
            secret_for_client_id(
                "new-id",
                None,
                Some("old-id".into()),
                Some("old-secret".into())
            ),
            None
        );
        assert_eq!(
            secret_for_client_id(
                "same-id",
                None,
                Some("same-id".into()),
                Some("saved".into())
            ),
            Some("saved".into())
        );
        assert_eq!(
            secret_for_client_id(
                "same-id",
                Some("explicit".into()),
                Some("same-id".into()),
                Some("saved".into())
            ),
            Some("explicit".into())
        );
    }

    #[test]
    fn authorize_url_includes_resource_and_pkce() {
        let url = build_authorize_url(
            "https://slack.com/oauth/v2_user/authorize",
            "cid",
            &["chat:write".into()],
            "challenge",
            "state123",
            "https://mcp.slack.com/mcp",
        )
        .unwrap();
        let parsed = Url::parse(&url).unwrap();
        let pairs: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs.get("client_id").unwrap(), "cid");
        assert_eq!(pairs.get("code_challenge_method").unwrap(), "S256");
        assert_eq!(pairs.get("resource").unwrap(), "https://mcp.slack.com/mcp");
        assert_eq!(pairs.get("scope").unwrap(), "chat:write");
        assert_eq!(pairs.get("redirect_uri").unwrap(), REDIRECT_URI);
    }
}
