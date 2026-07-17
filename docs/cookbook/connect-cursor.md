# Connect Cursor

1. Start Sumeru and confirm the badge shows **running**
2. On **Configure**, copy the endpoint URL and bearer token

![Configure tab with running badge, endpoint URL, and bearer token](/images/configure.png)

3. In Cursor MCP settings, add:

```json
{
  "mcpServers": {
    "sumeru": {
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
    "sumeru": {
      "command": "sumeru",
      "args": ["mcp-stdio"]
    }
  }
}
```

Install the CLI with `npm i -g sumeru` first (or point `SUMERU_BINARY` at a local build).
