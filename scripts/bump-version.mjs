/**
 * Bump project version across package/Cargo/README files.
 * Inputs: argv version (`0.2.12`) or `patch`|`minor`|`major`.
 * Outputs: rewritten version fields; prints old -> new.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const arg = process.argv[2];
if (!arg) {
  console.error("usage: npm run version:bump -- <version|patch|minor|major>");
  process.exit(1);
}

const tauriPath = path.join(root, "src-tauri/tauri.conf.json");
const current = JSON.parse(fs.readFileSync(tauriPath, "utf8")).version;
const next = resolveVersion(current, arg);
if (!/^\d+\.\d+\.\d+$/.test(next)) {
  console.error(`invalid version: ${next}`);
  process.exit(1);
}

/**
 * Inputs: current semver, bump kind or explicit version.
 * Outputs: next semver string.
 */
function resolveVersion(cur, kind) {
  if (/^\d+\.\d+\.\d+$/.test(kind)) return kind;
  const [maj, min, pat] = cur.split(".").map(Number);
  if (kind === "patch") return `${maj}.${min}.${pat + 1}`;
  if (kind === "minor") return `${maj}.${min + 1}.0`;
  if (kind === "major") return `${maj + 1}.0.0`;
  throw new Error(`unknown bump: ${kind}`);
}

/**
 * Inputs: relative path, mutator(obj)->void for JSON files.
 * Outputs: file rewritten with trailing newline.
 */
function editJson(rel, mutate) {
  const p = path.join(root, rel);
  const data = JSON.parse(fs.readFileSync(p, "utf8"));
  mutate(data);
  fs.writeFileSync(p, `${JSON.stringify(data, null, 2)}\n`);
}

editJson("package.json", (j) => {
  j.version = next;
});
editJson("package-lock.json", (j) => {
  j.version = next;
  if (j.packages?.[""]) j.packages[""].version = next;
});
editJson("packages/cli/package.json", (j) => {
  j.version = next;
});

/** Inputs: relative path. Outputs: first "version" field rewritten in place. */
function setJsonVersionField(rel) {
  const p = path.join(root, rel);
  fs.writeFileSync(
    p,
    fs
      .readFileSync(p, "utf8")
      .replace(/("version"\s*:\s*")[^"]+(")/, `$1${next}$2`),
  );
}

setJsonVersionField("src-tauri/tauri.conf.json");

const cargoToml = path.join(root, "src-tauri/Cargo.toml");
fs.writeFileSync(
  cargoToml,
  fs
    .readFileSync(cargoToml, "utf8")
    .replace(/^version = "[^"]+"/m, `version = "${next}"`),
);

const cargoLock = path.join(root, "src-tauri/Cargo.lock");
fs.writeFileSync(
  cargoLock,
  fs
    .readFileSync(cargoLock, "utf8")
    .replace(
      /(\[\[package\]\]\nname = "sumeru"\nversion = ")[^"]+(")/,
      `$1${next}$2`,
    ),
);

const readme = path.join(root, "README.md");
fs.writeFileSync(
  readme,
  fs
    .readFileSync(readme, "utf8")
    .replace(/\(currently `v[^`]+`\)/, `(currently \`v${next}\`)`),
);

console.log(`${current} -> ${next}`);
