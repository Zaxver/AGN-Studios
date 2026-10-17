//! Chat message types and conversation structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Role of a message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::Tool => write!(f, "tool"),
        }
    }
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    #[serde(default = "crate::new_id")]
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    /// Optional tool call data (for assistant messages with tool calls).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID (for tool response messages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Token usage statistics (for assistant messages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// Timestamp (ISO 8601).
    #[serde(default)]
    pub timestamp: String,
    /// Message metadata (e.g. attachments, tool results).
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Group ID for visual grouping (not folders, just visual).
    #[serde(default)]
    pub group_id: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: crate::new_id(),
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            usage: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
            group_id: String::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: crate::new_id(),
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            usage: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
            group_id: String::new(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: crate::new_id(),
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            usage: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
            group_id: String::new(),
        }
    }
}

/// A tool call from the assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    /// Result of the tool call (filled after execution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Whether the tool call was successful.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
}

/// Token usage statistics for a message.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    /// Time to first token (ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ms: Option<u32>,
    /// Generation speed (tokens/sec).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<f32>,
}

/// A conversation (chat session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    #[serde(default = "crate::new_id")]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    /// Visual group label (for grouping conversations visually).
    #[serde(default)]
    pub group: String,
    /// Created timestamp.
    #[serde(default)]
    pub created_at: String,
    /// Updated timestamp.
    #[serde(default)]
    pub updated_at: String,
    /// Bound workspace path (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

impl Conversation {
    pub fn new(title: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: crate::new_id(),
            title: title.into(),
            messages: Vec::new(),
            group: String::new(),
            created_at: now.clone(),
            updated_at: now,
            workspace: None,
        }
    }
}

/// Status indicator for a conversation (shown in sidebar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversationStatus {
    /// Green — generation complete.
    Idle,
    /// Yellow — waiting for user response (ask).
    WaitingUser,
    /// Spinning — generating response.
    Generating,
    /// Spinning with retry text — retrying keys.
    Retrying,
    /// Spinning with network text — network fluctuation.
    NetworkRetry,
    /// Red — error.
    Error,
}

/// Key status for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyStatus {
    /// Green — available.
    Available,
    /// Yellow — cooling down (429).
    Cooling,
    /// Yellow — network error.
    NetworkError,
    /// Red — invalid (401).
    Invalid,
    /// Gray — not yet used.
    Unused,
}

/// Key status info for the status panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStatusInfo {
    pub provider_id: String,
    pub provider_name: String,
    pub key_id: String,
    pub key_label: String,
    pub status: KeyStatus,
    /// Remaining cooldown seconds (if cooling).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_remaining: Option<u64>,
    /// Last error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Retry status shown during generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryStatus {
    /// "retrying" or "network" or "generating" or "waiting_user" or "error" or "idle"
    pub state: String,
    /// Current retry round (for retrying/network).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<u32>,
    /// Total rounds/keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    /// Error message (for error state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}