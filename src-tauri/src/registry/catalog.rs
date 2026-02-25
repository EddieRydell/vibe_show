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
        llm_hidden: info.llm_hidden,
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

/// Generate help text for LLM command discovery.
/// Three tiers: no topic → categories, category → command list, command → full schema.
pub fn help_text(topic: Option<&str>) -> String {
    let registry = command_registry();
    let visible: Vec<&CommandRegistryEntry> = registry.iter().filter(|e| !e.llm_hidden).collect();

    match topic {
        None => {
            // Tier 1: category overview (driven by CommandCategory::all() — exhaustive)
            let mut lines = vec!["Available command categories:".to_string()];
            for cat in CommandCategory::all() {
                let count = visible.iter().filter(|e| e.category == *cat).count();
                if count > 0 {
                    lines.push(format!("  {} ({count}) — {}", cat.slug(), cat.description()));
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
                .filter(|e| e.category.slug() == cat_lower)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_text_lists_all_categories() {
        let output = help_text(None);
        for cat in CommandCategory::all() {
            assert!(
                output.contains(cat.slug()),
                "help_text(None) missing category: {}",
                cat.slug()
            );
        }
    }

    #[test]
    fn help_text_python_returns_commands() {
        let output = help_text(Some("python"));
        assert!(
            output.contains("python commands:"),
            "help_text('python') should list python commands, got: {output}"
        );
        assert!(output.contains("get_python_status"));
    }

    #[test]
    fn help_text_agent_returns_commands() {
        let output = help_text(Some("agent"));
        assert!(
            output.contains("agent commands:"),
            "help_text('agent') should list agent commands, got: {output}"
        );
        assert!(output.contains("send_agent_message"));
    }
}
