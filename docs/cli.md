# CLI

Install the npm launcher (downloads the matching portable binary on first run):

```bash
npm i -g funnelit
funnelit doctor
funnelit mcp-stdio   # or just: funnelit
```

## Stdio MCP host config

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

## Overrides

- `FUNNELIT_BINARY` — local binary path
- `FUNNELIT_VERSION` — release version to download
- `FUNNELIT_CACHE_DIR` — cache directory for binaries

## Doctor

`funnelit doctor` reports CLI version, platform, cached binary, config dir writability, and whether something answers on `:7341/mcp`.
