import crypto from "node:crypto";
import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { assetName, releaseTag } from "./platform.js";
import { cacheDir, cachedBinaryPath } from "./cache.js";

const REPO = "PratyushChauhan/sumeru";

/**
 * Inputs: URL and optional redirect depth. Outputs: response body Buffer.
 */
function httpsGet(url, redirects = 0) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "User-Agent": "sumeru-npm-cli" } }, (res) => {
        const loc = res.headers.location;
        if (
          redirects < 5 &&
          loc &&
          [301, 302, 303, 307, 308].includes(res.statusCode)
        ) {
          res.resume();
          resolve(httpsGet(loc, redirects + 1));
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`GET ${url} → HTTP ${res.statusCode}`));
          return;
        }
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      })
      .on("error", reject);
  });
}

/**
 * Inputs: sha256 text file body and expected asset basename.
 * Outputs: lowercase hex digest or throws.
 */
export function parseSha256Sum(body, assetBasename) {
  const lines = String(body).split(/\r?\n/);
  for (const line of lines) {
    const m = line.trim().match(/^([a-fA-F0-9]{64})\s+\*?(\S+)$/);
    if (!m) continue;
    if (path.basename(m[2]) === assetBasename) return m[1].toLowerCase();
  }
  throw new Error(`sha256 for ${assetBasename} not found in checksum file`);
}

/**
 * Inputs: version and platform key. Outputs: path to verified cached binary.
 */
export async function ensureBinary(version, key) {
  if (process.env.SUMERU_BINARY) {
    return path.resolve(process.env.SUMERU_BINARY);
  }
  const dest = cachedBinaryPath(version, key);
  const name = assetName(version, key);
  const sumsName = `${name}.sha256`;
  const tag = releaseTag(version);
  const base = `https://github.com/${REPO}/releases/download/${tag}`;

  if (fs.existsSync(dest)) {
    return dest;
  }

  fs.mkdirSync(cacheDir(), { recursive: true });
  const tmp = `${dest}-${crypto.randomBytes(4).toString("hex")}.partial`;
  process.stderr.write(`sumeru: downloading ${name}…\n`);
  const [binBuf, sumBuf] = await Promise.all([
    httpsGet(`${base}/${name}`),
    httpsGet(`${base}/${sumsName}`),
  ]);
  const expected = parseSha256Sum(sumBuf.toString("utf8"), name);
  const actual = crypto.createHash("sha256").update(binBuf).digest("hex");
  if (actual !== expected) {
    throw new Error(
      `checksum mismatch for ${name}: got ${actual}, want ${expected}`,
    );
  }
  fs.writeFileSync(tmp, binBuf);
  fs.chmodSync(tmp, 0o755);
  fs.renameSync(tmp, dest);
  return dest;
}
