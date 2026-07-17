# CLI

Install the npm launcher (downloads the matching portable binary on first run):

```bash
npm i -g sumeru
sumeru doctor
sumeru mcp-stdio   # or just: sumeru
```

## Stdio MCP host config

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

## Overrides

- `SUMERU_BINARY` — local binary path
- `SUMERU_VERSION` — release version to download
- `SUMERU_CACHE_DIR` — cache directory for binaries

## Doctor

`sumeru doctor` reports CLI version, platform, cached binary, config dir writability, and whether something answers on `:7341/mcp`.
