# Getting started

1. Launch Funnelit. The funnel starts automatically and keeps running in the tray when you close the window.
2. Open **Configure**. Copy the endpoint URL and bearer token.

![Configure tab with running badge, endpoint URL, and bearer token](/images/configure.png)

3. Add MCPs via **Marketplace** (curated DCR HTTP MCPs) or **Configure → Add MCP** (stdio commands or HTTP MCP URLs — remote URLs need HTTPS; plain HTTP is loopback-only — see [OAuth](./oauth.md)).

![Marketplace tab with curated providers and Install buttons](/images/marketplace.png)

4. Point your MCP host (Cursor, etc.) at the funnel endpoint with the bearer token.

## Tray and pause

- Close the window → UI hides; endpoint keeps serving
- Tray → **Open** to show the UI again
- Tray → **Quit** to stop the endpoint
- Header **Pause** / **Resume** toggles the local funnel without quitting

## Autostart

Enable **Run at system startup** on the Configure tab so Funnelit launches hidden in the tray after login. Prefer enabling this from an installed/release build so the OS entry points at a stable binary path.
