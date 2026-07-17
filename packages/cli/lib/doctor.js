import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { cacheDir, cachedBinaryPath, isExecutableFile } from "./cache.js";
import { ensureBinary } from "./download.js";
import { packageVersion, platformKey } from "./platform.js";

/**
 * Inputs: none. Outputs: Funnelit config directory path (best effort).
 */
export function configDir() {
  if (process.platform === "darwin") {
    return path.join(
      os.homedir(),
      "Library",
      "Application Support",
      "com.funnelit.app",
      "funnelit",
    );
  }
  const xdg = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config");
  return path.join(xdg, "com.funnelit.app", "funnelit");
}

/**
 * Inputs: none. Outputs: Promise resolving to endpoint HTTP status or error string.
 */
function probeEndpoint() {
  return new Promise((resolve) => {
    const req = http.get(
      "http://127.0.0.1:7341/mcp",
      { timeout: 1500, headers: { Accept: "text/event-stream" } },
      (res) => {
        res.resume();
        resolve(`HTTP ${res.statusCode}`);
      },
    );
    req.on("timeout", () => {
      req.destroy();
      resolve("timeout");
    });
    req.on("error", (e) => resolve(e.code || e.message));
  });
}

/**
 * Inputs: none. Outputs: doctor report lines; exit-oriented ok flag.
 */
export async function runDoctor() {
  const version = process.env.FUNNELIT_VERSION || packageVersion();
  const key = platformKey();
  const lines = [];
  let ok = true;

  lines.push(`cli version: ${packageVersion()}`);
  lines.push(`platform: ${process.platform}/${process.arch} → ${key || "unsupported"}`);
  if (!key) {
    lines.push("binary: unsupported platform (need linux-x64, darwin-arm64, or darwin-x64)");
    return { ok: false, lines };
  }

  const cached = cachedBinaryPath(version, key);
  lines.push(`cache dir: ${cacheDir()}`);
  lines.push(`cached binary: ${cached} (${isExecutableFile(cached) ? "present" : "missing"})`);

  try {
    const bin = await ensureBinary(version, key);
    lines.push(`binary: ${bin}`);
  } catch (e) {
    ok = false;
    lines.push(`binary download: FAIL ${e.message || e}`);
  }

  const cfg = configDir();
  let cfgState = "missing";
  try {
    fs.mkdirSync(cfg, { recursive: true });
    fs.accessSync(cfg, fs.constants.W_OK);
    cfgState = fs.existsSync(path.join(cfg, "servers.json"))
      ? "writable (servers.json present)"
      : "writable";
  } catch (e) {
    ok = false;
    cfgState = `FAIL ${e.message || e}`;
  }
  lines.push(`config dir: ${cfg} (${cfgState})`);

  const endpoint = await probeEndpoint();
  lines.push(`endpoint :7341/mcp: ${endpoint}`);
  if (endpoint === "timeout" || String(endpoint).startsWith("ECONNREFUSED")) {
    lines.push("note: desktop funnel not running (ok for stdio-only use)");
  }

  return { ok, lines };
}
