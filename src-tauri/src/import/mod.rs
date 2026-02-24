pub mod vixen;
pub mod vixen_preview;

// ── Shared Vixen format constants ──────────────────────────────────
// Centralized so that callers (api.rs, commands.rs, registry handlers)
// don't hard-code file/directory names.

/// Directory inside a Vixen 3 profile containing system configuration.
pub const VIXEN_SYSTEM_DATA_DIR: &str = "SystemData";

/// Main Vixen system-config filename.
pub const VIXEN_SYSTEM_CONFIG_FILE: &str = "SystemConfig.xml";

/// Vixen sequence file extension.
pub const VIXEN_SEQUENCE_EXT: &str = "tim";

/// Vixen module-store filename (for preview data).
pub const VIXEN_MODULE_STORE_FILE: &str = "ModuleStore.xml";
