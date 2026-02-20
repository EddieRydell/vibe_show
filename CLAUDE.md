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
Effects are pure functions of (normalized time, pixel position, params) → Color. Built-in: Solid, Chase, Rainbow, Strobe, Gradient, Twinkle. Resolved from `EffectKind` enum via `resolve_effect()`. This trait is the extension point for future user-defined effects (DSL, WASM, LLM-generated).

**Engine (`engine/`)** — Frame evaluation pipeline:
1. For each track (bottom to top), find active effects at time `t`
2. Effects span across all targeted fixtures seamlessly (global pixel indexing)
3. Blend track outputs using `BlendMode` (Override, Add, Multiply, Max, Alpha)
4. Output: `Frame` = `HashMap<fixture_id, Vec<[r,g,b,a]>>`

**Tauri Commands (`commands.rs`)** — IPC bridge:
- `get_show()`, `get_frame(time)`, `play()`, `pause()`, `seek(time)`, `get_playback()`, `tick(dt)`
- State managed via `Mutex<Show>` and `Mutex<PlaybackState>`
- Frontend drives the animation loop via `tick(dt)` calls from `requestAnimationFrame`

### Frontend (`src/`)

**Styling**: Tailwind CSS v4 with design tokens defined in `src/index.css`. Light/dark mode via CSS variables under `:root` / `.dark`, mapped to Tailwind via `@theme inline`. Semantic classes: `bg-bg`, `bg-surface`, `bg-surface-2`, `border-border`, `text-text`, `text-text-2`, `bg-primary`, `text-primary`. Effect type colors use `fx-*` prefix. See `BRAND_GUIDE.md` for full token table and usage rules. Never use inline styles for colors — use the theme tokens.

**Components**:
- `App.tsx` — Layout: header → toolbar → main (sidebar + timeline) → collapsible preview
- `Toolbar.tsx` — Transport controls (play/pause/stop/skip) with SVG icons + time display
- `Timeline.tsx` — Main view: time ruler + track lanes with colored effect blocks + playhead
- `Preview.tsx` — Collapsible PixiJS 2D renderer for light preview
- `FixtureList.tsx` — Left sidebar showing fixtures and groups

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
