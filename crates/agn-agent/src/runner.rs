//! Agent runner — the main conversation loop.
//!
//! The loop:
//! 1. Build system message (prompt injection)
//! 2. Send messages to the LLM via the gateway
//! 3. If the response contains tool calls → execute them → append results → loop
//! 4. If the response is text → return to user
//! 5. If ask_user → pause and wait for user input
//! 6. Auto-compress context if it exceeds the threshold

use agn_core::{
    AgentSettings, AutoCompressSettings, ChatMessage, Conversation, MessageRole,
    PermissionLevel, RetryStatus, TokenUsage, ToolCall,
};
use crate::prompt::build_system_message;
use crate::tools::{ToolExecutor, ToolResult};
use crate::skills::SkillsManager;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Events emitted by the agent runner during generation.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A chunk of text was received from the LLM.
    TextChunk(String),
    /// The LLM is calling a tool.
    ToolCall(ToolCall),
    /// A tool call completed.
    ToolResult(String, ToolResult),
    /// A file write indicator (e.g. "↓1234").
    FileWrite(String),
    /// Status update (generating, retrying, network, waiting_user, error, idle).
    Status(RetryStatus),
    /// Generation complete with final usage stats.
    Done {
        content: String,
        usage: Option<TokenUsage>,
        tool_calls: Vec<ToolCall>,
    },
    /// Error.
    Error(String),
}

/// The agent runner.
pub struct AgentRunner {
    settings: AgentSettings,
    tool_executor: ToolExecutor,
    skills: SkillsManager,
    gateway_url: String,
}

impl AgentRunner {
    pub fn new(
        settings: AgentSettings,
        tool_executor: ToolExecutor,
        skills: SkillsManager,
        gateway_url: String,
    ) -> Self {
        Self {
            settings,
            tool_executor,
            skills,
            gateway_url,
        }
    }

    /// Run the generation loop for a conversation.
    pub async fn run(
        &self,
        conversation: &mut Conversation,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> anyhow::Result<()> {
        let max_steps = 256;

        // Emit "generating" status
        let _ = event_tx.send(AgentEvent::Status(RetryStatus {
            state: "generating".to_string(),
            current: None,
            total: None,
            message: Some("正在生成回复...".to_string()),
        }));

        // Build system message
        let character = if !self.settings.selected_character.is_empty() {
            self.settings.characters.iter().find(|c| c.id == self.settings.selected_character)
        } else {
            None
        };

        let enabled_skills = self.skills.enabled_skills();
        let system_text = build_system_message(
            &self.settings,
            character,
            &self.tool_executor,
            &enabled_skills,
        );

        // Build tool definitions
        let tool_defs = self.tool_executor.tool_definitions();
        let tools_json: Vec<serde_json::Value> = tool_defs.iter().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        }).collect();

        // Auto-compress if needed
        self.maybe_compress(conversation);

        for step in 0..max_steps {
            // Build messages array
            let mut messages: Vec<serde_json::Value> = Vec::new();
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_text,
            }));

            for msg in &conversation.messages {
                let mut msg_obj = serde_json::json!({
                    "role": msg.role.to_string(),
                    "content": msg.content,
                });
                // Add tool calls if present
                if let Some(tc) = &msg.tool_calls {
                    let calls: Vec<serde_json::Value> = tc.iter().map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "type": "function",
                            "function": {
                                "name": c.name,
                                "arguments": c.arguments.to_string(),
                            }
                        })
                    }).collect();
                    msg_obj["tool_calls"] = serde_json::json!(calls);
                }
                // Add tool call ID for tool response messages
                if let Some(id) = &msg.tool_call_id {
                    msg_obj["tool_call_id"] = serde_json::json!(id);
                }
                messages.push(msg_obj);
            }

            // Build request
            let model = self.settings.selected_model.clone();
            let mut req = serde_json::json!({
                "model": model,
                "messages": messages,
                "stream": true,
            });

            if !tools_json.is_empty() {
                req["tools"] = serde_json::json!(tools_json);
            }

            // Add thinking depth if set
            let thinking_depth = self.get_thinking_depth(&model);
            if thinking_depth > 0 {
                // Some models support "reasoning_effort" or "thinking" param
                req["reasoning_effort"] = serde_json::json!(match thinking_depth {
                    1 => "low",
                    2 => "medium",
                    3 => "high",
                    _ => "medium",
                });
            }

            // Send request to gateway
            let client = reqwest::Client::new();
            let url = format!("{}/v1/chat/completions", self.gateway_url.trim_end_matches('/'));

            let send_start = Instant::now();
            let resp = client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&req)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    // Stream response
                    let body = r.bytes().await.unwrap_or_default();
                    let text = String::from_utf8_lossy(&body);
                    let (content, tool_calls, usage) = parse_stream_response(&text, &send_start, &event_tx);

                    if !content.is_empty() {
                        let _ = event_tx.send(AgentEvent::TextChunk(content.clone()));
                    }

                    // If there are tool calls, execute them
                    if !tool_calls.is_empty() {
                        // Add assistant message with tool calls
                        let mut assistant_msg = ChatMessage::assistant(&content);
                        assistant_msg.tool_calls = Some(tool_calls.clone());
                        conversation.messages.push(assistant_msg);

                        // Execute each tool call
                        for tc in &tool_calls {
                            let _ = event_tx.send(AgentEvent::ToolCall(tc.clone()));

                            if tc.name == "ask_user" {
                                // Pause and wait for user input
                                let _ = event_tx.send(AgentEvent::Status(RetryStatus {
                                    state: "waiting_user".to_string(),
                                    current: None,
                                    total: None,
                                    message: Some("等待用户回复...".to_string()),
                                }));
                                // The runner will be re-invoked after user responds
                                return Ok(());
                            }

                            if tc.name == "todo_list" {
                                // Just acknowledge
                                let _ = event_tx.send(AgentEvent::ToolResult(
                                    tc.id.clone(),
                                    ToolResult::ok("Task list updated."),
                                ));
                                // Add tool response
                                conversation.messages.push(ChatMessage {
                                    id: agn_core::new_id(),
                                    role: MessageRole::Tool,
                                    content: "Task list updated.".to_string(),
                                    tool_calls: None,
                                    tool_call_id: Some(tc.id.clone()),
                                    usage: None,
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    metadata: Default::default(),
                                    group_id: String::new(),
                                });
                                continue;
                            }

                            // Execute the tool
                            let result = self.tool_executor.execute(tc).await;

                            // Emit file write indicator if present
                            if let Some(info) = &result.file_write_info {
                                let _ = event_tx.send(AgentEvent::FileWrite(info.clone()));
                            }

                            let _ = event_tx.send(AgentEvent::ToolResult(tc.id.clone(), result.clone()));

                            // Add tool response to conversation
                            let tool_response = ChatMessage {
                                id: agn_core::new_id(),
                                role: MessageRole::Tool,
                                content: result.output,
                                tool_calls: None,
                                tool_call_id: Some(tc.id.clone()),
                                usage: None,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                metadata: Default::default(),
                                group_id: String::new(),
                            };
                            conversation.messages.push(tool_response);
                        }

                        // Continue the loop for the next step
                        continue;
                    }

                    // No tool calls — generation is complete
                    let _ = event_tx.send(AgentEvent::Status(RetryStatus {
                        state: "idle".to_string(),
                        current: None,
                        total: None,
                        message: None,
                    }));

                    let _ = event_tx.send(AgentEvent::Done {
                        content: content.clone(),
                        usage: usage.clone(),
                        tool_calls: tool_calls.clone(),
                    });

                    // Add assistant message to conversation
                    let mut msg = ChatMessage::assistant(&content);
                    msg.usage = usage;
                    conversation.messages.push(msg);

                    return Ok(());
                }
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    error!("LLM request failed: {} {}", status, body);

                    if status.as_u16() == 429 {
                        let _ = event_tx.send(AgentEvent::Status(RetryStatus {
                            state: "error".to_string(),
                            current: None,
                            total: None,
                            message: Some("所有key token额度均用尽".to_string()),
                        }));
                        let _ = event_tx.send(AgentEvent::Error(format!("429: {}", body)));
                        return Err(anyhow::anyhow!("Rate limited: {}", body));
                    }

                    let _ = event_tx.send(AgentEvent::Error(format!("HTTP {}: {}", status, body)));
                    return Err(anyhow::anyhow!("HTTP {}: {}", status, body));
                }
                Err(e) => {
                    error!("Network error: {}", e);
                    let _ = event_tx.send(AgentEvent::Status(RetryStatus {
                        state: "network".to_string(),
                        current: Some(1),
                        total: Some(5),
                        message: Some(format!("网络波动...[1/5]")),
                    }));
                    let _ = event_tx.send(AgentEvent::Error(format!("网络异常: {}", e)));
                    return Err(anyhow::anyhow!("Network error: {}", e));
                }
            }
        }

        info!("Agent loop ended after {} steps", max_steps);
        Ok(())
    }

    /// Get thinking depth for a model.
    fn get_thinking_depth(&self, model: &str) -> u8 {
        if let Some(mp) = self.settings.model_params.iter().find(|m| m.model_id == model) {
            return mp.thinking_depth;
        }
        0
    }

    /// Auto-compress context if it exceeds the threshold.
    fn maybe_compress(&self, conversation: &mut Conversation) {
        let settings = &self.settings.auto_compress;
        if !settings.enabled {
            return;
        }

        let total_chars: usize = conversation.messages.iter().map(|m| m.content.len()).sum();
        // Rough estimate: ~4 chars per token
        let est_tokens = (total_chars / 4) as u32;

        if est_tokens > settings.threshold {
            info!("Auto-compressing context (est. {} tokens)", est_tokens);
            let keep = settings.keep_recent as usize;
            if conversation.messages.len() > keep {
                // Keep system messages + last N messages
                let system_count = conversation.messages.iter()
                    .filter(|m| m.role == MessageRole::System)
                    .count();
                let split_at = system_count.max(1);
                let drain_end = conversation.messages.len().saturating_sub(keep);

                if drain_end > split_at {
                    let drained: Vec<ChatMessage> = conversation.messages.drain(split_at..drain_end).collect();
                    let summary = summarize_messages(&drained);
                    let summary_msg = ChatMessage {
                        id: agn_core::new_id(),
                        role: MessageRole::System,
                        content: format!("[Context Summary]\n{}", summary),
                        tool_calls: None,
                        tool_call_id: None,
                        usage: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        metadata: Default::default(),
                        group_id: String::new(),
                    };
                    conversation.messages.insert(split_at, summary_msg);
                }
            }
        }
    }
}

/// Parse an SSE stream response.
fn parse_stream_response(
    text: &str,
    start: &Instant,
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
) -> (String, Vec<ToolCall>, Option<TokenUsage>) {
    let mut content = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut usage: Option<TokenUsage> = None;

    // Parse SSE lines
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" {
                break;
            }
            if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(data) {
                // Extract content delta
                if let Some(delta) = chunk.get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("delta")) {
                    if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
                        content.push_str(text);
                    }
                    // Extract tool calls
                    if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc in tcs {
                            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                            let func = tc.get("function").unwrap_or(&serde_json::Value::Null);
                            let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                            let args_str = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
                            let args: serde_json::Value = serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));

                            tool_calls.push(ToolCall {
                                id,
                                name,
                                arguments: args,
                                result: None,
                                success: None,
                            });
                        }
                    }
                }
                // Extract usage from final chunk
                if let Some(u) = chunk.get("usage") {
                    let elapsed = start.elapsed();
                    let prompt_tokens = u.get("prompt_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                    let completion_tokens = u.get("completion_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                    let total_tokens = u.get("total_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                    let tps = if completion_tokens > 0 && elapsed.as_secs_f32() > 0.0 {
                        Some(completion_tokens as f32 / elapsed.as_secs_f32())
                    } else {
                        None
                    };
                    usage = Some(TokenUsage {
                        prompt_tokens,
                        completion_tokens,
                        total_tokens,
                        time_to_first_token_ms: None,
                        tokens_per_second: tps,
                    });
                }
            }
        }
    }

    (content, tool_calls, usage)
}

/// Summarize a set of messages (simple implementation — just concatenates and truncates).
fn summarize_messages(messages: &[ChatMessage]) -> String {
    let mut summary = String::new();
    for msg in messages {
        let role = match msg.role {
            MessageRole::System => "System",
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::Tool => "Tool",
        };
        let truncated = if msg.content.len() > 500 {
            format!("{}...", &msg.content[..500])
        } else {
            msg.content.clone()
        };
        summary.push_str(&format!("{}: {}\n", role, truncated));
    }
    summary
}