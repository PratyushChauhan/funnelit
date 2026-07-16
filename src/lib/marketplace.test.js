import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  filterMarketplaceEntries,
  findInstalledServer,
  isInstalled,
  listMarketplaceEntries,
  marketplaceHost,
  marketplaceIdForUrl,
} from "./marketplace.js";

describe("marketplace helpers", () => {
  it("lists curated catalog entries", () => {
    const entries = listMarketplaceEntries();
    assert.deepEqual(
      entries.map((e) => e.id).sort(),
      [
        "canva",
        "fathom",
        "intercom",
        "linear",
        "neon",
        "notion",
        "sentry",
        "stripe",
      ],
    );
  });

  it("matches installed servers by HTTP URL", () => {
    const entry = { url: "https://api.fathom.ai/mcp" };
    const servers = [
      {
        id: "1",
        transport: { kind: "http", url: "https://api.fathom.ai/mcp" },
      },
    ];
    assert.equal(isInstalled(entry, servers), true);
    assert.equal(findInstalledServer(entry, servers)?.id, "1");
    assert.equal(isInstalled(entry, []), false);
  });

  it("filters by name description or url", () => {
    const entries = [
      { name: "Fathom", description: "meetings", url: "https://api.fathom.ai/mcp" },
      { name: "Linear", description: "issues", url: "https://mcp.linear.app/mcp" },
    ];
    assert.equal(filterMarketplaceEntries(entries, "fathom").length, 1);
    assert.equal(filterMarketplaceEntries(entries, "issues").length, 1);
    assert.equal(filterMarketplaceEntries(entries, "linear.app").length, 1);
    assert.equal(filterMarketplaceEntries(entries, "").length, 2);
  });

  it("returns hostname for display", () => {
    assert.equal(marketplaceHost("https://api.fathom.ai/mcp"), "api.fathom.ai");
    assert.equal(marketplaceHost("not-a-url"), "not-a-url");
  });

  it("resolves catalog id from MCP URL", () => {
    assert.equal(marketplaceIdForUrl("https://api.fathom.ai/mcp"), "fathom");
    assert.equal(marketplaceIdForUrl("https://mcp.linear.app/mcp"), "linear");
    assert.equal(marketplaceIdForUrl("https://example.com/mcp"), "");
  });
});

