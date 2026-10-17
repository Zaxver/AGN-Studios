//! Bidirectional translation between OpenAI Chat Completions and Anthropic Messages protocols.
//!
//! Adapted from sakura-llmgateway's protocol.rs, simplified for AGN's needs.

use serde_json::Value;

/// Wire protocol tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    OpenAi,
    Anthropic,
}

impl Protocol {
    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("anthropic") {
            Protocol::Anthropic
        } else {
            Protocol::OpenAi
        }
    }
}

/// Translate an Anthropic `/v1/messages` request body into an OpenAI
/// `/v1/chat/completions` body.
pub fn anthropic_to_openai_request(a: &Value) -> Value {
    let mut o = serde_json::Map::new();

    if let Some(m) = a.get("model").and_then(|m| m.as_str()) {
        o.insert("model".into(), Value::String(m.to_string()));
    }

    let max_tokens = a.get("max_tokens").and_then(|t| t.as_u64()).unwrap_or(4096);
    o.insert("max_tokens".into(), Value::Number(max_tokens.into()));

    if let Some(t) = a.get("temperature") {
        if !t.is_null() {
            o.insert("temperature".into(), t.clone());
        }
    }
    if let Some(t) = a.get("top_p") {
        if !t.is_null() {
            o.insert("top_p".into(), t.clone());
        }
    }
    if let Some(s) = a.get("stop_sequences").and_then(|s| s.as_array()) {
        o.insert("stop".into(), Value::Array(s.clone()));
    }
    if let Some(s) = a.get("stream") {
        o.insert("stream".into(), s.clone());
    }

    let mut messages: Vec<Value> = Vec::new();

    // system: string OR array of content blocks
    match a.get("system") {
        Some(Value::String(s)) if !s.is_empty() => {
            messages.push(serde_json::json!({ "role": "system", "content": s }));
        }
        Some(Value::Array(blocks)) => {
            let text: Vec<&str> = blocks
                .iter()
                .filter_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                        b.get("text").and_then(|t| t.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            if !text.is_empty() {
                messages.push(serde_json::json!({ "role": "system", "content": text.join("\n") }));
            }
        }
        _ => {}
    }

    // messages
    if let Some(msgs) = a.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = msg.get("content");
            match content {
                Some(Value::String(s)) => {
                    messages.push(serde_json::json!({ "role": role, "content": s }));
                }
                Some(Value::Array(blocks)) => {
                    // Convert content blocks to OpenAI format
                    let parts: Vec<Value> = blocks
                        .iter()
                        .filter_map(|b| {
                            let btype = b.get("type").and_then(|t| t.as_str())?;
                            match btype {
                                "text" => {
                                    let text = b.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                    Some(serde_json::json!({ "type": "text", "text": text }))
                                }
                                "image" => {
                                    let source = b.get("source")?;
                                    let media_type = source.get("media_type").and_then(|m| m.as_str())?;
                                    let data = source.get("data").and_then(|d| d.as_str())?;
                                    Some(serde_json::json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": format!("data:{};base64,{}", media_type, data)
                                        }
                                    }))
                                }
                                _ => None,
                            }
                        })
                        .collect();
                    messages.push(serde_json::json!({ "role": role, "content": parts }));
                }
                _ => {
                    messages.push(serde_json::json!({ "role": role, "content": "" }));
                }
            }
        }
    }

    o.insert("messages".into(), Value::Array(messages));

    // tools
    if let Some(tools) = a.get("tools").and_then(|t| t.as_array()) {
        let openai_tools: Vec<Value> = tools
            .iter()
            .filter_map(|t| {
                let name = t.get("name").and_then(|n| n.as_str())?;
                let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
                let schema = t.get("input_schema").cloned().unwrap_or(serde_json::json!({}));
                Some(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": desc,
                        "parameters": schema
                    }
                }))
            })
            .collect();
        if !openai_tools.is_empty() {
            o.insert("tools".into(), Value::Array(openai_tools));
        }
    }

    // tool_choice
    if let Some(tc) = a.get("tool_choice") {
        match tc.get("type").and_then(|t| t.as_str()) {
            Some("auto") => {
                o.insert("tool_choice".into(), Value::String("auto".into()));
            }
            Some("any") => {
                o.insert("tool_choice".into(), Value::String("required".into()));
            }
            Some("tool") => {
                let name = tc.get("name").and_then(|n| n.as_str()).unwrap_or("");
                o.insert(
                    "tool_choice".into(),
                    serde_json::json!({ "type": "function", "function": { "name": name } }),
                );
            }
            _ => {}
        }
    }

    Value::Object(o)
}

/// Translate an OpenAI `/v1/chat/completions` request body into an Anthropic
/// `/v1/messages` body.
pub fn openai_to_anthropic_request(o: &Value) -> Value {
    let mut a = serde_json::Map::new();

    if let Some(m) = o.get("model").and_then(|m| m.as_str()) {
        a.insert("model".into(), Value::String(m.to_string()));
    }

    let max_tokens = o.get("max_tokens").and_then(|t| t.as_u64()).unwrap_or(4096);
    a.insert("max_tokens".into(), Value::Number(max_tokens.into()));

    if let Some(t) = o.get("temperature") {
        if !t.is_null() {
            a.insert("temperature".into(), t.clone());
        }
    }
    if let Some(t) = o.get("top_p") {
        if !t.is_null() {
            a.insert("top_p".into(), t.clone());
        }
    }
    if let Some(s) = o.get("stop").and_then(|s| s.as_array()) {
        a.insert("stop_sequences".into(), Value::Array(s.clone()));
    }
    if let Some(s) = o.get("stream") {
        a.insert("stream".into(), s.clone());
    }

    let mut system_text = String::new();
    let mut messages: Vec<Value> = Vec::new();

    if let Some(msgs) = o.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = msg.get("content");

            if role == "system" {
                if let Some(Value::String(s)) = content {
                    if !system_text.is_empty() {
                        system_text.push_str("\n\n");
                    }
                    system_text.push_str(s);
                }
                continue;
            }

            let anthropic_role = if role == "assistant" { "assistant" } else { "user" };

            match content {
                Some(Value::String(s)) => {
                    messages.push(serde_json::json!({
                        "role": anthropic_role,
                        "content": [{ "type": "text", "text": s }]
                    }));
                }
                Some(Value::Array(parts)) => {
                    let blocks: Vec<Value> = parts
                        .iter()
                        .filter_map(|p| {
                            let ptype = p.get("type").and_then(|t| t.as_str())?;
                            match ptype {
                                "text" => {
                                    let text = p.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                    Some(serde_json::json!({ "type": "text", "text": text }))
                                }
                                "image_url" => {
                                    let url = p.get("image_url")
                                        .and_then(|u| u.get("url"))
                                        .and_then(|u| u.as_str())?;
                                    if let Some((media_type, data)) = url.strip_prefix("data:").and_then(|s| {
                                        let mut parts = s.splitn(2, ";base64,");
                                        let mt = parts.next()?;
                                        let d = parts.next()?;
                                        Some((mt.to_string(), d.to_string()))
                                    }) {
                                        Some(serde_json::json!({
                                            "type": "image",
                                            "source": { "type": "base64", "media_type": media_type, "data": data }
                                        }))
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            }
                        })
                        .collect();
                    messages.push(serde_json::json!({ "role": anthropic_role, "content": blocks }));
                }
                _ => {
                    messages.push(serde_json::json!({
                        "role": anthropic_role,
                        "content": [{ "type": "text", "text": "" }]
                    }));
                }
            }
        }
    }

    if !system_text.is_empty() {
        a.insert("system".into(), Value::String(system_text));
    }
    a.insert("messages".into(), Value::Array(messages));

    // tools
    if let Some(tools) = o.get("tools").and_then(|t| t.as_array()) {
        let anthropic_tools: Vec<Value> = tools
            .iter()
            .filter_map(|t| {
                let func = t.get("function")?;
                let name = func.get("name").and_then(|n| n.as_str())?;
                let desc = func.get("description").and_then(|d| d.as_str()).unwrap_or("");
                let schema = func.get("parameters").cloned().unwrap_or(serde_json::json!({}));
                Some(serde_json::json!({
                    "name": name,
                    "description": desc,
                    "input_schema": schema
                }))
            })
            .collect();
        if !anthropic_tools.is_empty() {
            a.insert("tools".into(), Value::Array(anthropic_tools));
        }
    }

    Value::Object(a)
}

/// Detect 429 error type from upstream response body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitType {
    /// "token plan .. exhausted" or "token plan entitlement exhausted"
    TokenPlanExhausted,
    /// "rpm exhausted" or "rate limit"
    RpmExhausted,
    /// "tpm exhausted"
    TpmExhausted,
    /// "inference exhausted"
    InferenceExhausted,
    /// Unrecognized 429
    Unknown,
}

/// Classify a 429 error from the response body.
pub fn classify_429(body: &str) -> RateLimitType {
    let lower = body.to_lowercase();
    if lower.contains("token plan") && (lower.contains("exhausted") || lower.contains("entitlement")) {
        return RateLimitType::TokenPlanExhausted;
    }
    if lower.contains("rpm") && lower.contains("exhausted") {
        return RateLimitType::RpmExhausted;
    }
    if lower.contains("tpm") && lower.contains("exhausted") {
        return RateLimitType::TpmExhausted;
    }
    if lower.contains("inference") && lower.contains("exhausted") {
        return RateLimitType::InferenceExhausted;
    }
    // Fallback: check for common rate limit keywords
    if lower.contains("rate limit") || lower.contains("too many requests") {
        return RateLimitType::RpmExhausted;
    }
    RateLimitType::Unknown
}

/// Determine the cooldown duration for a given rate limit type.
pub fn cooldown_for(
    rl: &RateLimitType,
    config: &agn_core::GatewayConfig,
) -> u64 {
    match rl {
        RateLimitType::TokenPlanExhausted => config.token_plan_cooldown_secs,
        RateLimitType::RpmExhausted | RateLimitType::TpmExhausted | RateLimitType::InferenceExhausted => {
            config.rpm_cooldown_secs
        }
        RateLimitType::Unknown => config.default_cooldown_secs,
    }
}