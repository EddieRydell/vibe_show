/**
 * Type-safe command return types, auto-derived from the generated CommandResult
 * discriminated union. No manual map — Rust is the single source of truth.
 *
 * When a Rust command variant is added/removed, `cargo test --lib` regenerates
 * CommandResult.ts, and these types update automatically.
 */

import type { CommandResult } from "../src-tauri/bindings/CommandResult";

// ── Core derived types ──────────────────────────────────────────────

/** All command name strings, extracted from the generated discriminated union. */
export type CommandName = CommandResult["command"];

/** Extract the `data` type for a specific command, or `null` for unit commands. */
export type CommandData<C extends CommandName> =
  Extract<CommandResult, { command: C }> extends { data: infer D } ? D : null;

/** Mapped type: command name → return data type. */
export type CommandReturnMap = {
  [K in CommandName]: CommandData<K>;
};

// ── Helper types ────────────────────────────────────────────────────

/** Commands that return data (non-null). */
export type DataCommand = {
  [K in CommandName]: CommandData<K> extends null ? never : K;
}[CommandName];

/** Commands that return null (unit). */
export type UnitCommand = {
  [K in CommandName]: CommandData<K> extends null ? K : never;
}[CommandName];
