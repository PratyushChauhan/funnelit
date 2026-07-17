# Add a stdio MCP

1. Open **Configure → Add MCP**
2. Set a name
3. In **Command or MCP URL**, paste the command (for example `npx` or a full binary path)
4. Add extra args one per line if needed
5. Put secrets as `KEY=value` lines under env — values go in the OS keychain
6. Click **Test**, then **Save**

Funnelit starts the process with argv only (no shell). `PATH` and `HOME` are passed through; other env vars must be listed explicitly.
