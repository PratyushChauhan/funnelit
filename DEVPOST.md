## Inspiration

MCP is exploding — Linear, Notion, Stripe, Sentry, and dozens of local tools all speak the same protocol. But every host still wants its own config: paste another URL, wire another bearer token, fight another OAuth dance. We wanted one local funnel: add N upstream MCPs once, then point Cursor (or any host) at a single authenticated endpoint and stop babysitting server lists.

## What it does

**Sumeru** is a local desktop MCP funnel. You add upstream servers (stdio commands like `npx …`, or Streamable HTTP URLs), and it exposes them through one loopback endpoint:

`http://127.0.0.1:7341/mcp` with bearer auth.

The host only sees three gateway tools:

- `list_mcps` — what’s configured
- `list_mcp_tools` — tools for a given upstream
- `execute_mcp_tool` — call an upstream tool

Extras that make it usable day to day:

- System tray lifecycle (close window → keep serving; Quit from tray to stop)
- Pause / Resume and optional launch at login
- Browser OAuth for HTTP MCPs (DCR when available; guided Client ID flow when not)
- Secrets in the OS keychain
- A Marketplace for one-click install of curated DCR providers (Notion, Linear, Stripe, Sentry, etc.)
- CLI + npm launcher (`sumeru-mcp`) for stdio hosts and `sumeru gui`

## How we built it

- **Desktop shell:** Tauri (Rust) + Svelte 5 / Vite / shadcn-svelte
- **Gateway:** Rust Streamable HTTP MCP server on loopback, bearer-gated, browser `Origin` rejected
- **Upstream pool:** lazy connect on first tool use; reuse until edit/delete/quit; no blind retries on ambiguous failures
- **Transports:** argv-based stdio (no shell strings) and HTTPS HTTP MCPs (plain HTTP only on loopback)
- **OAuth:** RFC 9728 / 8414 discovery, PKCE, Dynamic Client Registration, loopback callback on `:7342`
- **Distribution:** GitHub Releases (macOS/Linux), portable CLI binaries, npm `sumeru-mcp`, in-app VitePress docs

## Challenges we ran into

- **MCP + OAuth in the wild:** some providers support DCR cleanly; others need manual Client IDs, secrets, and exact redirect URIs — we had to support both without turning Configure into a form monster
- **Tool surface explosion:** dumping every upstream tool into the host breaks context; the three-tool gateway keeps the host schema small and routes intentionally
- **Desktop packaging reality:** AppImage + Wayland EGL quirks, keychain across platforms, tray vs. quit semantics, and keeping the funnel alive when the window closes
- **Security defaults:** loopback-only bind, required bearer token, argv-only stdio, and treating upstream metadata/output as untrusted

## Accomplishments that we're proud of

- A real product loop: install → Marketplace OAuth → paste one endpoint into Cursor → call tools
- OAuth that works for both DCR and non-DCR providers, with tokens in the keychain
- A tiny, intentional gateway API instead of a noisy tool dump
- Ship path: desktop UI, CLI/stdio mode, docs site, and release automation

## What we learned

- Aggregation is a product decision, not just a proxy — what you expose to the host matters as much as what you can connect
- OAuth “just works” is aspirational; discovery, DCR, and fallback UX all have to be first-class
- Local tooling wins when lifecycle is boring: tray, autostart, pause, keychain, one URL

## What's next for Sumeru

- Grow the Marketplace catalog and tighten non-DCR guided setup
- Better observability: connection health, last-error surfaces, safer retry policy where it’s unambiguous
- Smoother multi-host workflows and clearer agent-facing docs for the three gateway tools
- Cross-platform polish (Windows packaging, Wayland/AppImage edges) and more cookbook recipes
