import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

/**
 * Inputs: none. Outputs: CLI package version string.
 */
export function packageVersion() {
  return require("../package.json").version;
}

/**
 * Inputs: optional process.platform / process.arch overrides.
 * Outputs: release asset suffix like `linux-x64`, or null if unsupported.
 */
export function platformKey(platform = process.platform, arch = process.arch) {
  if (platform === "linux" && arch === "x64") return "linux-x64";
  if (platform === "darwin" && arch === "arm64") return "darwin-arm64";
  if (platform === "darwin" && arch === "x64") return "darwin-x64";
  return null;
}

/**
 * Inputs: version and platform key. Outputs: release asset basename.
 */
export function assetName(version, key) {
  return `funnelit-v${version}-${key}`;
}

/**
 * Inputs: version. Outputs: GitHub release tag.
 */
export function releaseTag(version) {
  return `v${version}`;
}
