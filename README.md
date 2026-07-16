# funnelit

Local desktop MCP funnel. Add N upstream MCP servers (stdio commands or HTTP URLs) and expose them through one authenticated Streamable HTTP endpoint.

## Run

```bash
npm run tauri dev
```

## Funnel endpoint

While the app is running and you click **Start**:

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

- **stdio**: executable path + args array + optional env secrets (keychain)
- **http**: paste any local/remote Streamable HTTP MCP URL + optional bearer/headers (keychain)

Plain HTTP is allowed only for loopback hosts. Remote URLs must use HTTPS.

## Lifecycle

- Upstream clients connect lazily on first `list_mcp_tools` / `execute_mcp_tool`
- Connections are reused until Funnelit stops, the MCP is edited/deleted, or the transport closes
- Tool execution is never auto-retried after an ambiguous failure

## Storage

- Config: app config dir `/funnelit/servers.json`
- Secrets: OS keychain service `funnelit` (endpoint token, env values, headers, bearer tokens)

## Security notes

- Funnel binds only to `127.0.0.1`
- Endpoint bearer token is required
- Stdio commands are argv-based (no shell strings)
- Upstream tool metadata/output is untrusted data
