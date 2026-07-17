# OAuth

For HTTP MCPs that advertise OAuth (RFC 9728 / 8414), Funnelit shows **Sign in** and opens the provider login page in your browser. Tokens are stored in the keychain.

## Paths

### DCR (Dynamic Client Registration)

When the authorization server supports DCR, **Sign in** registers a client automatically. No Client ID is required.

### No DCR

1. Create an OAuth / developer app with the provider
2. Register redirect URI exactly: `http://127.0.0.1:7342/oauth/callback`
3. Paste Client ID (and secret if required) under **Advanced**
4. Click **Sign in** again

The editor can open a ChatGPT guide with the sanitized MCP URL and required redirect URI filled in.

## Manual credentials

Bearer tokens and custom headers stay under **Advanced** when you are not using browser OAuth.

## URL rules

- Plain HTTP is allowed only for loopback hosts
- Remote URLs must use HTTPS
