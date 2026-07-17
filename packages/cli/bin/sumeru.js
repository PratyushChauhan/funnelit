#!/usr/bin/env node
import { spawn } from "node:child_process";
import { runDoctor } from "../lib/doctor.js";
import { ensureBinary } from "../lib/download.js";
import { packageVersion, platformKey } from "../lib/platform.js";

/**
 * Inputs: argv (without node/script). Outputs: process exit via spawn or print.
 */
async function main(argv) {
  if (argv.includes("--version") || argv.includes("-V")) {
    process.stdout.write(`${packageVersion()}\n`);
    return;
  }

  if (argv[0] === "doctor") {
    const { ok, lines } = await runDoctor();
    for (const line of lines) process.stdout.write(`${line}\n`);
    process.exitCode = ok ? 0 : 1;
    return;
  }

  const version = process.env.SUMERU_VERSION || packageVersion();
  const key = platformKey();
  if (!key) {
    throw new Error(
      `unsupported platform ${process.platform}/${process.arch} (need linux-x64, darwin-arm64, or darwin-x64)`,
    );
  }

  const bin = await ensureBinary(version, key);
  const pass =
    argv.length === 0 || argv[0] === "mcp-stdio"
      ? ["mcp-stdio", ...argv.slice(argv[0] === "mcp-stdio" ? 1 : 0)]
      : argv;

  const child = spawn(bin, pass, { stdio: "inherit" });
  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
  child.on("error", (err) => {
    console.error(`sumeru: failed to spawn ${bin}: ${err.message}`);
    process.exit(1);
  });
}

main(process.argv.slice(2)).catch((err) => {
  console.error(`sumeru: ${err.message || err}`);
  process.exit(1);
});
