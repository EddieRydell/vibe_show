//! LLM provider abstraction — builds HTTP requests and normalizes responses
//! for both Anthropic and OpenAI-compatible APIs.

use serde_json::Value;

use crate::settings::{LlmProvider, LlmProviderConfig};

/// Resolved provider details ready for making an API call.
pub struct ResolvedProvider {
    pub url: String,
    pub api_key: String,
    pub model: String,
    pub provider: LlmProvider,
}

impl ResolvedProvider {
    /// Resolve a provider config into concrete URL / key / model values.
    ///
    /// # Errors
    /// Returns an error string if the API key is missing.
    pub fn from_config(config: &LlmProviderConfig) -> Result<Self, String> {
        let api_key = config
            .api_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .ok_or("No API key configured. Set it in Settings.")?
            .to_string();

        match config.provider {
            LlmProvider::Anthropic => Ok(Self {
                url: "https://api.anthropic.com/v1/messages".to_string(),
                api_key,
                model: config
                    .model
                    .clone()
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
                provider: LlmProvider::Anthropic,
            }),
            LlmProvider::OpenAiCompatible => {
                let base = config
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.openai.com/v1");
                let base = base.trim_end_matches('/');
                Ok(Self {
                    url: format!("{base}/chat/completions"),
                    api_key,
                    model: config
                        .model
                        .clone()
                        .unwrap_or_else(|| "gpt-4o".to_string()),
                    provider: LlmProvider::OpenAiCompatible,
                })
            }
        }
    }
}

/// Build an HTTP request (method, url, headers, body) for the resolved provider.
pub fn build_request(
    client: &reqwest::Client,
    provider: &ResolvedProvider,
    system_prompt: &str,
    messages: &[Value],
    tools: &Value,
) -> reqwest::RequestBuilder {
    match provider.provider {
        LlmProvider::Anthropic => build_anthropic_request(
            client,
            provider,
            system_prompt,
            messages,
            tools,
        ),
        LlmProvider::OpenAiCompatible => build_openai_request(
            client,
            provider,
            system_prompt,
            messages,
            tools,
        ),
    }
}

/// Parse the provider's HTTP response JSON into Anthropic-shaped content blocks
/// and a stop reason, so the rest of chat.rs can stay provider-agnostic.
///
/// Returns `(content_blocks, stop_reason)`.
///
/// # Errors
/// Returns an error string on parse failure.
pub fn parse_response(
    provider: &LlmProvider,
    json: &Value,
) -> Result<(Vec<Value>, String), String> {
    match provider {
        LlmProvider::Anthropic => Ok(parse_anthropic_response(json)),
        LlmProvider::OpenAiCompatible => parse_openai_response(json),
    }
}

// ── Anthropic ────────────────────────────────────────────────────

fn build_anthropic_request(
    client: &reqwest::Client,
    provider: &ResolvedProvider,
    system_prompt: &str,
    messages: &[Value],
    tools: &Value,
) -> reqwest::RequestBuilder {
    let body = serde_json::json!({
        "model": provider.model,
        "max_tokens": 4096,
        "system": system_prompt,
        "tools": tools,
        "messages": messages,
    });

    client
        .post(&provider.url)
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
}

fn parse_anthropic_response(json: &Value) -> (Vec<Value>, String) {
    let stop_reason = json
        .get("stop_reason")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let blocks = json
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    (blocks, stop_reason)
}

// ── OpenAI-compatible ────────────────────────────────────────────

fn build_openai_request(
    client: &reqwest::Client,
    provider: &ResolvedProvider,
    system_prompt: &str,
    messages: &[Value],
    tools: &Value,
) -> reqwest::RequestBuilder {
    // Convert Anthropic-shaped messages to OpenAI format
    let mut oai_messages: Vec<Value> = Vec::new();

    // System prompt as a system message
    oai_messages.push(serde_json::json!({
        "role": "system",
        "content": system_prompt,
    }));

    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        let content = &msg["content"];

        match role {
            "user" => {
                if let Some(text) = content.as_str() {
                    oai_messages.push(serde_json::json!({
                        "role": "user",
                        "content": text,
                    }));
                } else if let Some(blocks) = content.as_array() {
                    // Check if these are tool_result blocks (sent as role=user in Anthropic)
                    let mut has_tool_results = false;
                    for block in blocks {
                        if block.get("type").and_then(Value::as_str) == Some("tool_result") {
                            has_tool_results = true;
                            oai_messages.push(serde_json::json!({
                                "role": "tool",
                                "tool_call_id": block.get("tool_use_id").and_then(Value::as_str).unwrap_or(""),
                                "content": block.get("content").and_then(Value::as_str).unwrap_or(""),
                            }));
                        }
                    }
                    if !has_tool_results {
                        // Plain text blocks from user
                        let text: String = blocks
                            .iter()
                            .filter_map(|b| b.get("text").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("\n");
                        if !text.is_empty() {
                            oai_messages.push(serde_json::json!({
                                "role": "user",
                                "content": text,
                            }));
                        }
                    }
                }
            }
            "assistant" => {
                if let Some(blocks) = content.as_array() {
                    let mut text_parts = Vec::new();
                    let mut tool_calls = Vec::new();

                    for block in blocks {
                        match block.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                if let Some(t) = block.get("text").and_then(Value::as_str) {
                                    text_parts.push(t.to_string());
                                }
                            }
                            Some("tool_use") => {
                                tool_calls.push(serde_json::json!({
                                    "id": block.get("id").and_then(Value::as_str).unwrap_or(""),
                                    "type": "function",
                                    "function": {
                                        "name": block.get("name").and_then(Value::as_str).unwrap_or(""),
                                        "arguments": serde_json::to_string(
                                            block.get("input").unwrap_or(&Value::Null)
                                        ).unwrap_or_default(),
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    let mut msg = serde_json::Map::new();
                    msg.insert("role".to_string(), Value::String("assistant".to_string()));
                    if !text_parts.is_empty() {
                        msg.insert("content".to_string(), Value::String(text_parts.join("\n")));
                    }
                    if !tool_calls.is_empty() {
                        msg.insert("tool_calls".to_string(), Value::Array(tool_calls));
                    }
                    oai_messages.push(Value::Object(msg));
                } else if let Some(text) = content.as_str() {
                    oai_messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": text,
                    }));
                }
            }
            _ => {}
        }
    }

    // Convert Anthropic tool definitions to OpenAI format
    let oai_tools: Vec<Value> = tools
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|tool| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": tool.get("name").and_then(Value::as_str).unwrap_or(""),
                            "description": tool.get("description").and_then(Value::as_str).unwrap_or(""),
                            "parameters": tool.get("input_schema").unwrap_or(&Value::Null),
                        }
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let body = serde_json::json!({
        "model": provider.model,
        "max_tokens": 4096,
        "messages": oai_messages,
        "tools": oai_tools,
    });

    client
        .post(&provider.url)
        .header("Authorization", format!("Bearer {}", provider.api_key))
        .header("content-type", "application/json")
        .json(&body)
}

fn parse_openai_response(json: &Value) -> Result<(Vec<Value>, String), String> {
    let choice = json
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .ok_or("No choices in OpenAI response")?;

    let finish_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .unwrap_or("stop");

    let message = choice
        .get("message")
        .ok_or("No message in OpenAI choice")?;

    let mut blocks = Vec::new();

    // Text content
    if let Some(text) = message.get("content").and_then(Value::as_str) {
        if !text.is_empty() {
            blocks.push(serde_json::json!({
                "type": "text",
                "text": text,
            }));
        }
    }

    // Tool calls → tool_use blocks
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tc in tool_calls {
            let func = tc.get("function").unwrap_or(&Value::Null);
            let arguments_str = func
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let input: Value =
                serde_json::from_str(arguments_str).unwrap_or(Value::Null);

            blocks.push(serde_json::json!({
                "type": "tool_use",
                "id": tc.get("id").and_then(Value::as_str).unwrap_or(""),
                "name": func.get("name").and_then(Value::as_str).unwrap_or(""),
                "input": input,
            }));
        }
    }

    // Map OpenAI finish_reason to Anthropic stop_reason
    let stop_reason = match finish_reason {
        "tool_calls" => "tool_use".to_string(),
        "stop" => "end_turn".to_string(),
        "length" => "max_tokens".to_string(),
        other => other.to_string(),
    };

    Ok((blocks, stop_reason))
}
