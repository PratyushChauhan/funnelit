# OAuth without DCR

Use this when Sign in says the server does not support automatic registration.

1. Open **Configure → Add MCP** (or edit an existing HTTP MCP)
2. Paste the HTTPS MCP URL
3. Under **Advanced**, note the required redirect URI: `http://127.0.0.1:7342/oauth/callback`
4. Optionally use **Open guide** to get provider-specific OAuth app steps
5. Create the provider app, register that redirect URI, copy Client ID (and secret if required)
6. Paste credentials under **Advanced**
7. Click **Sign in** and finish in the browser

Tokens are stored in the keychain. Re-sign-in if the provider revokes access.
