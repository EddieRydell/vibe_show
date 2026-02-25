#!/usr/bin/env node
/**
 * Cross-platform prebuild script for Tauri production builds.
 *
 * 1. Downloads the `uv` binary for the current platform (if not present)
 * 2. Builds the frontend (`pnpm build`)
 * 3. Compiles the agent sidecar (`bun run build:compile`)
 * 4. Stages agent-sidecar artifacts into src-tauri/resources/agent-sidecar/
 * 5. Removes wrong-platform uv binaries
 *
 * Invoked automatically by Tauri via `beforeBuildCommand`.
 */
import { execSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  unlinkSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const RESOURCES = join(ROOT, "src-tauri", "resources");
const SIDECAR_ROOT = join(ROOT, "agent-sidecar");
const SIDECAR_DIST = join(SIDECAR_ROOT, "dist");
const SIDECAR_DEST = join(RESOURCES, "agent-sidecar");

const isWindows = process.platform === "win32";
const isMac = process.platform === "darwin";
const isLinux = process.platform === "linux";

const UV_VERSION = "0.6.6";

function run(cmd, opts = {}) {
  console.log(`\n> ${cmd}`);
  execSync(cmd, { stdio: "inherit", ...opts });
}

// ─── 1. Download uv if needed ───────────────────────────────────────────────

function downloadUv() {
  const binaryName = isWindows ? "uv.exe" : "uv";
  const dest = join(RESOURCES, binaryName);

  if (existsSync(dest)) {
    console.log(`[prebuild] uv already present at ${dest}`);
    return;
  }

  mkdirSync(RESOURCES, { recursive: true });

  const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
  let triple, ext;

  if (isWindows) {
    triple = `${arch}-pc-windows-msvc`;
    ext = "zip";
  } else if (isMac) {
    triple = `${arch}-apple-darwin`;
    ext = "tar.gz";
  } else {
    triple = `${arch}-unknown-linux-gnu`;
    ext = "tar.gz";
  }

  const archive = `uv-${triple}.${ext}`;
  const url = `https://github.com/astral-sh/uv/releases/download/${UV_VERSION}/${archive}`;
  const tmpDir = join(RESOURCES, ".uv-tmp");

  mkdirSync(tmpDir, { recursive: true });

  console.log(`[prebuild] Downloading uv ${UV_VERSION} (${triple})...`);
  const archivePath = join(tmpDir, archive);
  run(`curl -fSL "${url}" -o "${archivePath}"`);

  console.log("[prebuild] Extracting uv...");
  if (ext === "zip") {
    // Windows zip: uv.exe is at the archive root
    run(`tar -xf "${archivePath}" -C "${tmpDir}"`);
  } else {
    // macOS/Linux tar.gz: binary is inside uv-{triple}/uv
    run(`tar xzf "${archivePath}" -C "${tmpDir}" --strip-components=1`);
  }

  // Find the binary in tmpDir
  const extracted = findFile(tmpDir, binaryName);
  if (!extracted) {
    throw new Error(`Could not find ${binaryName} in extracted archive`);
  }

  cpSync(extracted, dest);
  if (!isWindows) {
    execSync(`chmod +x "${dest}"`);
  }

  rmSync(tmpDir, { recursive: true, force: true });
  console.log(`[prebuild] uv installed at ${dest}`);
}

function findFile(dir, name) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      const found = findFile(full, name);
      if (found) return found;
    } else if (entry.name === name) {
      return full;
    }
  }
  return null;
}

// ─── 2. Build frontend ─────────────────────────────────────────────────────

function buildFrontend() {
  console.log("[prebuild] Building frontend...");
  run("pnpm build", { cwd: ROOT });
}

// ─── 3. Compile agent sidecar ───────────────────────────────────────────────

function compileSidecar() {
  console.log("[prebuild] Compiling agent sidecar...");
  run("bun run build:compile", { cwd: SIDECAR_ROOT });
}

// ─── 4. Stage agent-sidecar artifacts ───────────────────────────────────────

function stageSidecar() {
  console.log("[prebuild] Staging agent-sidecar artifacts...");
  mkdirSync(SIDECAR_DEST, { recursive: true });

  const binaryName = isWindows ? "agent-sidecar.exe" : "agent-sidecar";

  // Copy binary
  cpSync(join(SIDECAR_DIST, binaryName), join(SIDECAR_DEST, binaryName));
  if (!isWindows) {
    execSync(`chmod +x "${join(SIDECAR_DEST, binaryName)}"`);
  }

  // Copy cli.js
  cpSync(join(SIDECAR_DIST, "cli.js"), join(SIDECAR_DEST, "cli.js"));

  // Copy WASM files
  for (const wasm of ["resvg.wasm", "tree-sitter.wasm", "tree-sitter-bash.wasm"]) {
    const src = join(SIDECAR_DIST, wasm);
    if (existsSync(src)) {
      cpSync(src, join(SIDECAR_DEST, wasm));
    }
  }

  // Copy vendor directory (ripgrep binaries)
  const vendorSrc = join(SIDECAR_DIST, "vendor");
  if (existsSync(vendorSrc)) {
    cpSync(vendorSrc, join(SIDECAR_DEST, "vendor"), { recursive: true });
  }

  console.log("[prebuild] Agent sidecar staged.");
}

// ─── 5. Remove wrong-platform uv binary ────────────────────────────────────

function cleanupWrongPlatformUv() {
  if (isWindows) {
    // Remove Unix binary if present
    const unix = join(RESOURCES, "uv");
    if (existsSync(unix)) unlinkSync(unix);
  } else {
    // Remove Windows binary if present
    const win = join(RESOURCES, "uv.exe");
    if (existsSync(win)) unlinkSync(win);
  }
}

// ─── Run ────────────────────────────────────────────────────────────────────

try {
  downloadUv();
  buildFrontend();
  compileSidecar();
  stageSidecar();
  cleanupWrongPlatformUv();
  console.log("\n[prebuild] All done!");
} catch (err) {
  console.error("\n[prebuild] FAILED:", err.message);
  process.exit(1);
}
