# funnelit

Local desktop MCP funnel. Add N upstream MCP servers (stdio commands or HTTP URLs) and expose them through one authenticated Streamable HTTP endpoint.

## Run

Frontend is Svelte 5 + Vite + [shadcn-svelte](https://www.shadcn-svelte.com/) (Rhea / mist). Dev/build are wired through Tauri:

```bash
npm install
npm run tauri dev
```

CI (`.github/workflows/build.yml`) builds macOS (arm64 + x64) and Linux on push to `main`, PRs, or manual dispatch, and uploads the installers as artifacts.

## Funnel endpoint

The funnel starts automatically on launch and keeps running in the system tray when you close the window. Open from the tray to configure; Quit from the tray to stop the endpoint. Optionally enable **Run at system startup** (starts hidden in the tray). Pause/Resume in the UI if needed:

- URL: `http://127.0.0.1:7341/mcp`
- Auth: `Authorization: Bearer <token>` (shown/copied from the UI)
- Browser `Origin` requests are rejected

Example client config:

```json
{
  "mcpServers": {
    "funnelit": {
      "url": "http://127.0.0.1:7341/mcp",
      "headers": {
        "Authorization": "Bearer <token>"
      }
    }
  }
}
```

## Gateway tools

Funnelit exposes exactly three MCP tools:

| Tool | Inputs | Outputs |
| --- | --- | --- |
| `list_mcps` | none | configured MCP ids, names, transports, enabled flags |
| `list_mcp_tools` | `mcp_id` | upstream tool names, descriptions, schemas |
| `execute_mcp_tool` | `mcp_id`, `tool_name`, `arguments?` | upstream `CallToolResult` |

## Upstream MCP formats

- **stdio**: paste a command (e.g. `npx`) plus args/env secrets (keychain)
- **http**: paste a Streamable HTTP MCP URL

For HTTP MCPs that advertise OAuth (RFC 9728 / 8414), funnelit shows **Sign in** and opens the provider login page in your browser. Tokens are stored in the keychain.

OAuth paths:

- **DCR** — when the authorization server supports Dynamic Client Registration, Sign in registers a client automatically (no Client ID needed)
- **No DCR** — the UI shows a short guide: create an OAuth app with the provider, register redirect URI `http://127.0.0.1:7342/oauth/callback`, paste Client ID (and secret if required) under **Advanced**, then Sign in

Manual bearer/headers also stay under **Advanced**.

Plain HTTP is allowed only for loopback hosts. Remote URLs must use HTTPS.

## Lifecycle

- Closing the window hides the UI; the MCP funnel keeps serving from the tray
- Upstream clients connect lazily on first `list_mcp_tools` / `execute_mcp_tool`
- Connections are reused until Funnelit quits, the MCP is edited/deleted, or the transport closes
- Tool execution is never auto-retried after an ambiguous failure

## Storage

- Config: app config dir `/funnelit/servers.json`
- Secrets: OS keychain service `funnelit` (endpoint token, env/header/bearer values, OAuth client + refresh tokens)

## Security notes

- Funnel binds only to `127.0.0.1`
- Endpoint bearer token is required
- Stdio commands are argv-based (no shell strings)
- Upstream tool metadata/output is untrusted data
