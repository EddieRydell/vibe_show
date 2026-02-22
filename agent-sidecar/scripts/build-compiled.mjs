#!/usr/bin/env node
/**
 * Build a standalone compiled agent-sidecar binary using bun build --compile.
 * Copies cli.js and native assets alongside the binary.
 */
import { execSync } from "node:child_process";
import { cpSync, mkdirSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const DIST = join(ROOT, "dist");
const SDK = join(ROOT, "node_modules", "@anthropic-ai", "claude-agent-sdk");

// Detect platform
const isWindows = process.platform === "win32";
const binaryName = isWindows ? "agent-sidecar.exe" : "agent-sidecar";

mkdirSync(DIST, { recursive: true });

// 1. Compile
console.log("[build] Compiling with bun...");
execSync(
  `bun build --compile --minify src/index.ts "${join(SDK, "cli.js")}" --outfile "${join(DIST, binaryName)}"`,
  { cwd: ROOT, stdio: "inherit" },
);

// 2. Copy cli.js alongside binary (the SDK spawns this as a subprocess)
console.log("[build] Copying cli.js...");
cpSync(join(SDK, "cli.js"), join(DIST, "cli.js"));

// 3. Copy WASM files
for (const wasm of ["resvg.wasm", "tree-sitter.wasm", "tree-sitter-bash.wasm"]) {
  const src = join(SDK, wasm);
  if (existsSync(src)) {
    console.log(`[build] Copying ${wasm}...`);
    cpSync(src, join(DIST, wasm));
  }
}

// 4. Copy platform-specific ripgrep binaries
const platformMap = {
  "win32-x64": "x64-win32",
  "darwin-arm64": "arm64-darwin",
  "darwin-x64": "x64-darwin",
  "linux-x64": "x64-linux",
  "linux-arm64": "arm64-linux",
};
const platformKey = `${process.platform}-${process.arch}`;
const ripgrepDir = platformMap[platformKey];
if (ripgrepDir) {
  const src = join(SDK, "vendor", "ripgrep", ripgrepDir);
  if (existsSync(src)) {
    const dest = join(DIST, "vendor", "ripgrep", ripgrepDir);
    console.log(`[build] Copying ripgrep (${ripgrepDir})...`);
    mkdirSync(dirname(dest), { recursive: true });
    cpSync(src, dest, { recursive: true });
  }
}

console.log(`[build] Done! Binary: ${join(DIST, binaryName)}`);
