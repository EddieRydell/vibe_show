import { build } from "esbuild";

await build({
  entryPoints: ["src/index.ts"],
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node20",
  outfile: "dist/index.mjs",
  banner: {
    js: 'import { createRequire } from "module"; const require = createRequire(import.meta.url);',
  },
  external: [
    // Don't bundle native modules
    "fsevents",
    // Keep SDK external — it spawns cli.js as a subprocess and needs
    // to resolve it relative to its own package directory
    "@anthropic-ai/claude-agent-sdk",
  ],
});
