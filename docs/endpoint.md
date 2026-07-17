# Endpoint

- **URL:** `http://127.0.0.1:7341/mcp`
- **Auth:** `Authorization: Bearer <token>` (copy from Configure)
- Browser `Origin` requests are rejected
- Transport: stateless Streamable HTTP with JSON POSTs; GET `/mcp` returns a keep-alive SSE stream

## Example client config

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

## Lifecycle

- Upstream clients connect lazily on first `list_mcp_tools` / `execute_mcp_tool`
- Connections are reused until Funnelit quits, the MCP is edited/deleted, or the transport closes
- Tool execution is never auto-retried after an ambiguous failure

## Storage and security

- Config: app config dir `/funnelit/servers.json`
- Secrets: OS keychain service `funnelit`
- Funnel binds only to `127.0.0.1`
- Stdio commands are argv-based (no shell strings)
- Upstream tool metadata/output is untrusted data
