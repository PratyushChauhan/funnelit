import { defineConfig } from "vitepress";

const rawBase = process.env.DOCS_BASE || "/";
const base = rawBase === "/" ? "/" : rawBase.replace(/\/?$/, "/");

/** Inputs: none. Outputs: VitePress site config for sumeru docs. */
export default defineConfig({
  title: "sumeru",
  description: "Local desktop MCP funnel",
  // Local/app: `/`. GitHub Pages project site: `DOCS_BASE=/sumeru/`.
  base,
  srcDir: ".",
  outDir: "../src-tauri/resources/docs",
  cleanUrls: true,
  appearance: "dark",
  head: [
    [
      "link",
      {
        rel: "icon",
        type: "image/svg+xml",
        href: `${base}images/sumeru-mark.svg`,
      },
    ],
  ],
  themeConfig: {
    logo: {
      light: "/images/sumeru-mark.svg",
      dark: "/images/sumeru-mark-light.svg",
      alt: "Sumeru",
    },
    nav: [
      { text: "Guides", link: "/" },
      {
        text: "GitHub",
        link: "https://github.com/PratyushChauhan/sumeru",
      },
    ],
    sidebar: [
      {
        text: "Guides",
        items: [
          { text: "Overview", link: "/" },
          { text: "Getting started", link: "/getting-started" },
          { text: "Endpoint", link: "/endpoint" },
          { text: "OAuth", link: "/oauth" },
          { text: "Marketplace", link: "/marketplace" },
          { text: "CLI", link: "/cli" },
        ],
      },
      {
        text: "Cookbook",
        items: [
          { text: "Connect Cursor", link: "/cookbook/connect-cursor" },
          { text: "Add a stdio MCP", link: "/cookbook/add-stdio-mcp" },
          { text: "OAuth without DCR", link: "/cookbook/oauth-without-dcr" },
          {
            text: "Install from Marketplace",
            link: "/cookbook/install-from-marketplace",
          },
          { text: "Run at startup", link: "/cookbook/run-at-startup" },
          { text: "Rotate the endpoint token", link: "/cookbook/rotate-token" },
        ],
      },
      {
        text: "Linux",
        items: [
          { text: "Install on Linux", link: "/cookbook/linux/install" },
          {
            text: "Run at startup",
            link: "/cookbook/linux/run-at-startup",
          },
          {
            text: "Fix AppImage on Wayland",
            link: "/cookbook/linux/appimage-wayland",
          },
          {
            text: "AppImage on Hyprland",
            link: "/cookbook/linux/appimage-hyprland",
          },
        ],
      },
      {
        text: "macOS",
        items: [
          { text: "Install on macOS", link: "/cookbook/macos/install" },
          {
            text: "Run at startup",
            link: "/cookbook/macos/run-at-startup",
          },
        ],
      },
    ],
    search: { provider: "local" },
  },
});
