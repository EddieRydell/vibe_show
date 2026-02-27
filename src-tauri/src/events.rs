//! Single source of truth for event names shared between Rust and TypeScript.
//! The TypeScript mirror is `src/events.ts` — a sync test verifies they match.

pub const CHAT_TOKEN: &str = "chat:token";
pub const CHAT_TOOL_CALL: &str = "chat:tool_call";
pub const CHAT_TOOL_RESULT: &str = "chat:tool_result";
pub const CHAT_COMPLETE: &str = "chat:complete";
pub const CHAT_THINKING: &str = "chat:thinking";
pub const CHAT_ERROR: &str = "chat:error";
pub const PROGRESS: &str = "progress";
pub const SHOW_REFRESHED: &str = "show-refreshed";
pub const SELECTION_CHANGED: &str = "selection-changed";

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Verify that every constant in this module has a matching export in `src/events.ts`.
    #[test]
    fn events_sync_with_typescript() {
        let ts_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("src")
            .join("events.ts");
        let ts_source = std::fs::read_to_string(&ts_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", ts_path.display()));

        let rust_events = [
            ("CHAT_TOKEN", CHAT_TOKEN),
            ("CHAT_TOOL_CALL", CHAT_TOOL_CALL),
            ("CHAT_TOOL_RESULT", CHAT_TOOL_RESULT),
            ("CHAT_COMPLETE", CHAT_COMPLETE),
            ("CHAT_THINKING", CHAT_THINKING),
            ("CHAT_ERROR", CHAT_ERROR),
            ("PROGRESS", PROGRESS),
            ("SHOW_REFRESHED", SHOW_REFRESHED),
            ("SELECTION_CHANGED", SELECTION_CHANGED),
        ];

        for (name, value) in &rust_events {
            let expected = format!("\"{}\"", value);
            assert!(
                ts_source.contains(&expected),
                "TypeScript events.ts missing {name} = {expected}"
            );
        }
    }
}
