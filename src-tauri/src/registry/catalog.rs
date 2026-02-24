#![allow(clippy::needless_pass_by_value)]

use schemars::schema_for;
use serde::Serialize;
use serde_json::Value;

use super::{CommandCategory, CommandInfo};

/// A registry entry: metadata + JSON schema for the params.
#[derive(Debug, Clone, Serialize)]
pub struct CommandRegistryEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub category: CommandCategory,
    pub undoable: bool,
    pub llm_hidden: bool,
    pub param_schema: Value,
}

pub(super) fn empty_object_schema() -> Value {
    serde_json::json!({ "type": "object", "properties": {} })
}

pub(super) fn schema_value<T: schemars::JsonSchema>() -> Value {
    let root = schema_for!(T);
    serde_json::to_value(root).unwrap_or(empty_object_schema())
}

pub(super) fn entry(info: CommandInfo, param_schema: Value) -> CommandRegistryEntry {
    CommandRegistryEntry {
        name: info.name,
        description: info.description,
        category: info.category,
        undoable: info.undoable,
        llm_hidden: info.is_llm_hidden(),
        param_schema,
    }
}

pub(super) fn de<T: serde::de::DeserializeOwned>(input: &Value) -> Result<T, String> {
    serde_json::from_value(input.clone()).map_err(|e| e.to_string())
}

/// The complete command registry, auto-generated from param struct schemas.
pub fn command_registry() -> Vec<CommandRegistryEntry> {
    super::Command::registry_entries()
}

/// Generate the minimal `tools` array for LLM chat.
/// Instead of dumping all command schemas (which blows past token limits),
/// we expose just 3 meta-tools: `help`, `run`, and `batch`.
/// The LLM discovers specific commands via `help` and invokes them via `run`/`batch`.
pub fn to_llm_tools() -> Value {
    serde_json::json!([
        {
            "name": "help",
            "description": "Discover available commands. No args = list categories. Provide a category name to see its commands, or a command name to see its full parameter schema.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "Category name (e.g. 'edit') or command name (e.g. 'add_effect')"
                    }
                }
            }
        },
        {
            "name": "run",
            "description": "Execute a single command. Use help() first to discover command names and parameters.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Command name (e.g. 'add_effect', 'get_show')" },
                    "params": { "type": "object", "description": "Command parameters (see help for schema)" }
                },
                "required": ["command"]
            }
        },
        {
            "name": "batch",
            "description": "Execute multiple edit commands as a single undoable operation. Each entry has 'command' and optional 'params'.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "description": { "type": "string", "description": "Human-readable description of the batch" },
                    "commands": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" },
                                "params": { "type": "object" }
                            },
                            "required": ["command"]
                        }
                    }
                },
                "required": ["description", "commands"]
            }
        }
    ])
}

/// Generate help text for LLM command discovery.
/// Three tiers: no topic → categories, category → command list, command → full schema.
pub fn help_text(topic: Option<&str>) -> String {
    let registry = command_registry();
    let visible: Vec<&CommandRegistryEntry> = registry.iter().filter(|e| !e.llm_hidden).collect();

    match topic {
        None => {
            // Tier 1: category overview
            let categories = [
                ("edit", CommandCategory::Edit, "Add, delete, move effects and tracks, update params"),
                ("playback", CommandCategory::Playback, "Play, pause, seek, undo, redo"),
                ("query", CommandCategory::Query, "Inspect show state and effect types"),
                ("analysis", CommandCategory::Analysis, "Audio analysis: beats, sections, mood"),
                ("library", CommandCategory::Library, "Manage gradients, curves, scripts"),
                ("script", CommandCategory::Script, "Write and compile DSL scripts"),
                ("settings", CommandCategory::Settings, "App settings and data directory"),
                ("setup", CommandCategory::Setup, "Setup CRUD: list, create, open, delete"),
                ("sequence", CommandCategory::Sequence, "Sequence CRUD: list, create, open, delete"),
                ("media", CommandCategory::Media, "Audio file management"),
                ("chat", CommandCategory::Chat, "Chat history management"),
                ("import", CommandCategory::Import, "Vixen 3 project import"),
            ];

            let mut lines = vec!["Available command categories:".to_string()];
            for (name, cat, desc) in &categories {
                let count = visible.iter().filter(|e| e.category == *cat).count();
                if count > 0 {
                    lines.push(format!("  {name} ({count}) — {desc}"));
                }
            }
            lines.push(String::new());
            lines.push("Use help({topic: \"edit\"}) to list commands in a category.".to_string());
            lines.push("Use help({topic: \"add_effect\"}) for full parameter details.".to_string());
            lines.join("\n")
        }
        Some(topic) => {
            // Try as command name first (tier 3: full schema)
            if let Some(entry) = visible.iter().find(|e| e.name == topic) {
                let schema_str = serde_json::to_string_pretty(&entry.param_schema)
                    .unwrap_or_else(|_| "{}".to_string());
                return format!(
                    "{}: {}\nCategory: {:?} | Undoable: {}\n\nParameters:\n{}",
                    entry.name,
                    entry.description,
                    entry.category,
                    if entry.undoable { "yes" } else { "no" },
                    schema_str,
                );
            }

            // Try as category name (tier 2: command list)
            let cat_lower = topic.to_lowercase();
            let matching: Vec<&&CommandRegistryEntry> = visible
                .iter()
                .filter(|e| {
                    format!("{:?}", e.category).to_lowercase() == cat_lower
                })
                .collect();

            if matching.is_empty() {
                format!(
                    "Unknown topic: \"{topic}\". Use help() to see categories and commands."
                )
            } else {
                let mut lines = vec![format!("{topic} commands:")];
                for entry in &matching {
                    lines.push(format!("  - {}: {}", entry.name, entry.description));
                }
                lines.push(String::new());
                lines.push(
                    "Use help({topic: \"command_name\"}) for parameter details.".to_string(),
                );
                lines.join("\n")
            }
        }
    }
}

/// Generate JSON Schema formatted tool list (for MCP / REST).
pub fn to_json_schema() -> Value {
    Value::Array(
        command_registry()
            .iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "description": e.description,
                    "category": e.category,
                    "undoable": e.undoable,
                    "inputSchema": e.param_schema,
                })
            })
            .collect(),
    )
}

/// Deserialize a tool call (name + JSON input) into a Command.
pub fn deserialize_from_tool_call(name: &str, input: &Value) -> Result<super::Command, String> {
    super::Command::from_tool_call(name, input)
}
