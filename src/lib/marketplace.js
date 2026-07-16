import catalog from "./marketplace.json" with { type: "json" };

/**
 * Inputs: none. Outputs: marketplace catalog entries array.
 */
export function listMarketplaceEntries() {
  return catalog.entries ?? [];
}

/**
 * Inputs: catalog entry, configured servers. Outputs: matching server or null.
 */
export function findInstalledServer(entry, servers) {
  const url = entry?.url?.trim();
  if (!url) return null;
  return (
    (servers || []).find(
      (s) => s?.transport?.kind === "http" && s.transport.url === url,
    ) || null
  );
}

/**
 * Inputs: catalog entry, configured servers. Outputs: true when URL is installed.
 */
export function isInstalled(entry, servers) {
  return !!findInstalledServer(entry, servers);
}

/**
 * Inputs: entries, search query. Outputs: entries matching name/description/url.
 */
export function filterMarketplaceEntries(entries, query) {
  const q = (query || "").trim().toLowerCase();
  if (!q) return entries || [];
  return (entries || []).filter((e) => {
    const hay = `${e.name || ""} ${e.description || ""} ${e.url || ""}`.toLowerCase();
    return hay.includes(q);
  });
}

/**
 * Inputs: catalog URL. Outputs: hostname for display, or the URL.
 */
export function marketplaceHost(url) {
  try {
    return new URL(url).hostname;
  } catch {
    return url || "";
  }
}

/**
 * Inputs: MCP URL. Outputs: catalog entry id when host/url matches, else "".
 */
export function marketplaceIdForUrl(url) {
  const trimmed = (url || "").trim();
  if (!trimmed) return "";
  const entries = listMarketplaceEntries();
  const exact = entries.find((e) => e.url === trimmed);
  if (exact) return exact.id;
  const host = marketplaceHost(trimmed);
  if (!host) return "";
  return entries.find((e) => marketplaceHost(e.url) === host)?.id || "";
}
