import fs from "node:fs";
import os from "node:os";
import path from "node:path";

/**
 * Inputs: none. Outputs: directory for cached Sumeru binaries.
 */
export function cacheDir() {
  if (process.env.SUMERU_CACHE_DIR) {
    return path.resolve(process.env.SUMERU_CACHE_DIR);
  }
  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Caches", "sumeru");
  }
  const xdg = process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache");
  return path.join(xdg, "sumeru");
}

/**
 * Inputs: version, platform key. Outputs: absolute path for cached binary.
 */
export function cachedBinaryPath(version, key) {
  return path.join(cacheDir(), `sumeru-v${version}-${key}`);
}

/**
 * Inputs: file path. Outputs: true when path is an executable file.
 */
export function isExecutableFile(filePath) {
  try {
    fs.accessSync(filePath, fs.constants.X_OK);
    return fs.statSync(filePath).isFile();
  } catch {
    return false;
  }
}
