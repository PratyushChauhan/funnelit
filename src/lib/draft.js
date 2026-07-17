const OAUTH_REDIRECT = "http://127.0.0.1:7342/oauth/callback";

/**
 * Inputs: KEY=value text. Outputs: object map or throws on malformed lines.
 */
export function parsePairs(text) {
  const out = {};
  for (const [index, line] of text.split("\n").entries()) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const i = trimmed.indexOf("=");
    if (i <= 0) {
      throw new Error(`Invalid KEY=value entry on line ${index + 1}`);
    }
    out[trimmed.slice(0, i).trim()] = trimmed.slice(i + 1);
  }
  return out;
}

/**
 * Inputs: connection string. Outputs: true when it looks like an MCP URL.
 */
export function isHttpSource(value) {
  return /^https?:\/\//i.test(value.trim());
}

/**
 * Inputs: command line text. Outputs: { command, args } after a simple split.
 */
export function splitCommandLine(text) {
  const parts = text.trim().match(/(?:[^\s"]+|"[^"]*")+/g) || [];
  const tokens = parts.map((p) => p.replace(/^"|"$/g, ""));
  return { command: tokens[0] || "", args: tokens.slice(1) };
}

/**
 * Inputs: form fields and oauthConnected. Outputs: transport + secrets draft.
 */
export function readDraft({
  source,
  headersText,
  bearer,
  envText,
  argsText,
  oauthConnected,
}) {
  const trimmed = source.trim();
  if (isHttpSource(trimmed)) {
    const headers = parsePairs(headersText);
    const bearerVal = bearer.trim();
    return {
      transport: {
        kind: "http",
        url: trimmed,
        header_keys: Object.keys(headers),
        has_bearer: oauthConnected || !!bearerVal,
      },
      secrets: { env: {}, headers, bearer: bearerVal || null },
    };
  }
  const env = parsePairs(envText);
  const argLines = argsText
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
  const parsed = splitCommandLine(trimmed);
  return {
    transport: {
      kind: "stdio",
      command: parsed.command,
      args: argLines.length ? parsed.args.concat(argLines) : parsed.args,
      env_keys: Object.keys(env),
    },
    secrets: { env, headers: {}, bearer: null },
  };
}

/**
 * Inputs: MCP URL. Outputs: origin+path only, or null when the URL cannot be parsed safely.
 */
export function sanitizeMcpUrlForGuide(mcpUrl) {
  try {
    const parsed = new URL(mcpUrl.trim());
    parsed.username = "";
    parsed.password = "";
    parsed.search = "";
    parsed.hash = "";
    return parsed.toString();
  } catch {
    return null;
  }
}

/**
 * Inputs: MCP URL. Outputs: ChatGPT URL with a setup prompt for non-DCR OAuth.
 */
export function chatgptOauthGuideUrl(mcpUrl) {
  const safeUrl = sanitizeMcpUrlForGuide(mcpUrl);
  const urlLine = safeUrl
    ? `Sanitized MCP URL shared with you (query, fragment, and credentials removed): ${safeUrl}`
    : "MCP URL was not shared (could not sanitize it safely).";
  const prompt = [
    "I am connecting an MCP server to sumeru (a local MCP funnel desktop app).",
    "This server does not support OAuth Dynamic Client Registration.",
    "",
    urlLine,
    `Required OAuth redirect URI (register exactly): ${OAUTH_REDIRECT}`,
    "",
    "Give step-by-step instructions to:",
    "1. Create the right OAuth / developer app for this provider",
    "2. Enable MCP or the scopes this MCP needs",
    "3. Register that redirect URI",
    "4. Find the Client ID and Client secret (if any)",
    "5. What to paste into sumeru Advanced → OAuth fields, then click Sign in",
    "",
    "Be concrete for this exact URL/provider. Keep steps short.",
  ].join("\n");
  return `https://chatgpt.com/?q=${encodeURIComponent(prompt)}`;
}
