# Connect Cursor

1. Start Funnelit and confirm the badge shows **running**
2. On **Configure**, copy the endpoint URL and bearer token

![Configure tab with running badge, endpoint URL, and bearer token](/images/configure.png)

3. In Cursor MCP settings, add:

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

4. Reload MCP servers in Cursor
5. Call `list_mcps` to confirm your configured upstreams appear

## Stdio alternative

If you prefer stdio instead of HTTP:

```json
{
  "mcpServers": {
    "funnelit": {
      "command": "funnelit",
      "args": ["mcp-stdio"]
    }
  }
}
```

Install the CLI with `npm i -g funnelit` first (or point `FUNNELIT_BINARY` at a local build).
