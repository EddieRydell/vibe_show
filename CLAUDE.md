# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VibeLights is an AI-integrated light show sequencer (think XLights/Vixen but with AI generation). Built with Tauri v2: Rust backend for the engine, React/TypeScript frontend for the UI.

## Build & Dev Commands

```bash
# Install dependencies
pnpm install

# Development (launches Tauri window with hot reload)
pnpm tauri dev

# Production build
pnpm tauri build

# TypeScript check
pnpm check

# ESLint
pnpm lint

# Knip (unused exports/deps)
pnpm knip

# Copy/paste detection (TypeScript + Rust)
pnpm cpd

# Rust check (faster than full build)
cd src-tauri && cargo check

# Rust lint
cd src-tauri && cargo clippy

# Rust tests
cd src-tauri && cargo test

# Format check
npx prettier --check src/
```

Rust requires `$HOME/.cargo/bin` in PATH. If cargo isn't found, run: `export PATH="$HOME/.cargo/bin:$PATH"`

## Architecture

### Rust Backend (`src-tauri/src/`)

**Data Model (`model/`)** — Type-safe, serde-serializable types. All newtypes use `#[serde(transparent)]` for clean JSON serialization.
- `Color` (RGBA with blend ops via `std::ops::Add`, lerp, multiply, max, over, scale, HSV)
- `FixtureDef` — logical fixture definition. Separated from physical output (patching is a separate concern).
- `Patch` + `OutputMapping` — maps fixtures to physical outputs (DMX universe/address, or pixel controller port). Supports `ChannelOrder` (RGB, GRB, BRG, etc.) for different LED chip types.
- `Controller` + `ControllerProtocol` — physical controllers (E1.31, ArtNet, Serial).
- `Show` → `Sequence` → `Track` → `EffectInstance` (the timeline hierarchy)
- `TimeRange` (validated: start < end, both non-negative — `TimeRange::new()` returns `Option`)
- `DmxAddress` (validated 1-512 range via `DmxAddress::new()` → `Option`)
- `EffectParams` (HashMap<String, ParamValue> with typed accessor helpers like `float_or`, `color_or`)

**Effect System (`effects/`)** — The core abstraction:
```rust
pub trait Effect: Send + Sync {
    fn evaluate(&self, t: f64, pixel_index: usize, pixel_count: usize, params: &EffectParams) -> Color;
    fn name(&self) -> &'static str;
}
```
Effects are pure functions of (normalized time, pixel position, params) → Color. Built-in effects defined by `EffectKind` enum in `model/timeline.rs` (Solid, Chase, Rainbow, Strobe, Gradient, Twinkle, Fade, Wipe at time of writing). Resolved via `resolve_effect()`. This trait is the extension point for user-defined effects via the DSL `Script` variant.

**Engine (`engine/`)** — Frame evaluation pipeline:
1. For each track (bottom to top), find active effects at time `t`
2. Effects span across all targeted fixtures seamlessly (global pixel indexing)
3. Blend track outputs using `BlendMode` enum (see `model/timeline.rs` for full list)
4. Output: `Frame` = `HashMap<fixture_id, Vec<[r,g,b,a]>>`

**Command Registry (`registry/`)** — Unified IPC layer. A single `define_commands!` macro in `mod.rs` defines every operation as a variant of the `Command` enum (`#[serde(tag="command", content="params")]`). The macro auto-generates: the `Command` and `CommandResult` enums (both ts-rs exported), dispatch functions (sync + async), JSON schemas (via schemars), help text, and deserialization from tool calls. Adding a new command = add a param struct in `params.rs`, add a variant to the macro, implement a handler — the compiler enforces the rest.
- `catalog.rs` — Three-tier help system (categories → commands → parameter schemas), `to_json_schema()` for REST/MCP introspection, `to_llm_tools()` for agent discovery
- `execute.rs` — Exhaustive match dispatch; compiler error if any variant is unhandled
- `params.rs` — All parameter structs with `#[derive(JsonSchema)]` for auto schema generation
- `handlers/` — Handler modules by domain (edit, playback, query, analysis, library, script, settings, setup, sequence, media, chat, import, python, agent, etc.)
- `reference.rs` — Auto-generated DSL language reference and design guide (blend modes, beat sync, color theory)
- Tauri exposes just two commands: `exec` (dispatches any `Command`) and `get_command_registry` (introspection)
- State managed via `Mutex<Show>` and `Mutex<PlaybackState>`; frontend drives animation via `tick(dt)` from `requestAnimationFrame`

**Audit Logging (`audit.rs`)** — Every agent tool execution is logged to `{app_config_dir}/agent-logs/YYYY-MM-DD.jsonl`. Captures timestamp, conversation ID, tool name, input params, success/failure, result message, wall-clock duration, and scratch file path. Best-effort append — never panics or fails the caller.

**Agent Chat (`chat.rs`)** — Tool execution bridge for the agent sidecar (`execute_tool_api()`), multi-conversation persistence (`AgentChatsData` in `agent-chats.json`), and the `ChatEmitter` trait for streaming events to the frontend.

### Frontend (`src/`)

**Styling**: Tailwind CSS v4 with design tokens defined in `src/index.css`. Light/dark mode via CSS variables under `:root` / `.dark`, mapped to Tailwind via `@theme inline`. Semantic classes: `bg-bg`, `bg-surface`, `bg-surface-2`, `border-border`, `text-text`, `text-text-2`, `bg-primary`, `text-primary`. Effect type colors use `fx-*` prefix. See `BRAND_GUIDE.md` for full token table and usage rules. Never use inline styles for colors — use the theme tokens.

**Components**:
- `App.tsx` — Layout: nav bar + screen router
- `Toolbar.tsx` — Transport controls (play/pause/stop/skip) with SVG icons + time display
- `Timeline.tsx` — Main view: time ruler + track lanes with colored effect blocks + playhead
- `PropertyPanel.tsx` — Effect parameter editor sidebar
- `LibraryPanel.tsx` — Global gradients, curves, scripts sidebar
- `ChatPanel.tsx` — AI agent chat panel
- `TabBar.tsx` — Tab navigation with setup indicator

**Screens** (`screens/`):
- `HomeScreen.tsx` — Tabbed hub (Effects, Gradients, Curves, Sequences, Music, House Setup, Layout)
- `EditorScreen.tsx` — Timeline editor with toolbar + preview
- `ScriptScreen.tsx` — DSL script editor with live preview
- `AnalysisScreen.tsx` — Audio analysis workspace
- `SettingsScreen.tsx` — App settings
- `DetachedPreview.tsx` — Standalone preview window

**Hooks**:
- `useEngine.ts` — Wraps all Tauri IPC (show loading, playback control, frame updates via requestAnimationFrame tick loop)

**Types** (auto-generated via [ts-rs](https://github.com/Aleph-Alpha/ts-rs)):
- `src-tauri/bindings/` — Generated TypeScript types from Rust (`#[derive(TS)]` + `#[ts(export)]`). **Do not edit manually.** Regenerate with `pnpm bindings` (or `cd src-tauri && cargo test export_bindings`).
- `src/types.ts` — Barrel file re-exporting generated bindings + frontend-only types (`BULB_SHAPE_RADIUS`, `InteractionMode`).
- **When you change a Rust type that has `#[derive(TS)]`, you must run `pnpm bindings` to regenerate.** TypeScript will catch mismatches at compile time (`pnpm check`).

### Design Principles

- **Make illegal states unrepresentable**: newtypes (`FixtureId`, `GroupId`, `DmxAddress`), validated constructors returning `Option`, exhaustive enums
- **Single source of truth**: brand colors in Tailwind theme, Rust types as canonical model, TS types auto-generated via ts-rs
- **Effects are pure functions**: no mutation, no RNG state — same inputs always produce same output
- **Separation of concerns**: fixture definitions (what) are separate from patching (where) and controllers (how)
- **Composition over configuration**: effects compose via blend modes on tracks, not via complex parameter hierarchies
- **Keep extensibility in mind**: the `Effect` trait and `EffectParams` are designed so user-defined effects (DSL/WASM/LLM) slot in naturally
