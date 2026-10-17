//! Tool calling system for the agent.
//!
//! Available tools (permission-dependent):
//! - terminal: Execute terminal commands (bash on Linux, PowerShell on Windows, Termux on Android)
//! - file_write: Write content to a file
//! - file_read: Read content from a file
//! - file_list: List files in a directory
//! - file_delete: Delete a file or directory
//! - skill: Execute a skill
//! - ask_user: Ask the user a question (pauses generation, shows yellow light)
//! - todo_list: Create and manage a task list

use agn_core::{AgnError, AgnResult, PermissionLevel, AndroidPermission, ToolCall};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info, warn};

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    /// File write indicator (e.g. "↓1234" shown in UI when writing files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_write_info: Option<String>,
}

impl ToolResult {
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
            file_write_info: None,
        }
    }

    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error.into()),
            file_write_info: None,
        }
    }

    pub fn file_write(bytes: usize, path: &str) -> Self {
        Self {
            success: true,
            output: format!("Written {} bytes to {}", bytes, path),
            error: None,
            file_write_info: Some(format!("↓{}", bytes)),
        }
    }
}

/// Tool definition for the LLM (OpenAI function-calling format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// The tool executor — dispatches tool calls based on permission level.
pub struct ToolExecutor {
    pub permission: PermissionLevel,
    pub android_permission: AndroidPermission,
    pub workspace: Option<PathBuf>,
    pub is_android: bool,
}

impl ToolExecutor {
    pub fn new(permission: PermissionLevel) -> Self {
        let is_android = cfg!(target_os = "android");
        Self {
            permission,
            android_permission: AndroidPermission::Normal,
            workspace: None,
            is_android,
        }
    }

    pub fn with_workspace(mut self, ws: Option<PathBuf>) -> Self {
        self.workspace = ws;
        self
    }

    pub fn with_android_permission(mut self, p: AndroidPermission) -> Self {
        self.android_permission = p;
        self
    }

    /// Get all available tool definitions for the LLM.
    pub fn tool_definitions(&self) -> Vec<ToolDef> {
        let mut tools = Vec::new();

        // Terminal — only with full access or higher permissions
        if self.can_use_terminal() {
            tools.push(ToolDef {
                name: "terminal".to_string(),
                description: "Execute a terminal command. On Linux/macOS uses bash, on Windows uses PowerShell, on Android uses Termux. Returns stdout, stderr, and exit code.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The terminal command to execute"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Timeout in seconds (default 30)",
                            "default": 30
                        }
                    },
                    "required": ["command"]
                }),
            });
        }

        // File operations
        tools.push(ToolDef {
            name: "file_write".to_string(),
            description: "Write content to a file. Creates parent directories if needed. Shows write progress in UI.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write to" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            }),
        });

        tools.push(ToolDef {
            name: "file_read".to_string(),
            description: "Read content from a file.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read" }
                },
                "required": ["path"]
            }),
        });

        tools.push(ToolDef {
            name: "file_list".to_string(),
            description: "List files in a directory.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path to list" }
                },
                "required": ["path"]
            }),
        });

        tools.push(ToolDef {
            name: "file_delete".to_string(),
            description: "Delete a file or directory.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to delete" }
                },
                "required": ["path"]
            }),
        });

        // Ask user
        tools.push(ToolDef {
            name: "ask_user".to_string(),
            description: "Ask the user a question and wait for their response. Use this when you need clarification or user input to proceed.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "The question to ask the user" },
                    "options": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional list of choices for the user"
                    }
                },
                "required": ["question"]
            }),
        });

        // Todo list
        tools.push(ToolDef {
            name: "todo_list".to_string(),
            description: "Create or update a task list to track progress on multi-step tasks.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "task": { "type": "string", "description": "Task description" },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "Task status"
                                }
                            },
                            "required": ["task", "status"]
                        },
                        "description": "List of tasks with their statuses"
                    }
                },
                "required": ["tasks"]
            }),
        });

        // Skill execution
        tools.push(ToolDef {
            name: "skill".to_string(),
            description: "Execute an installed skill. Skills are extensions that provide specialized capabilities.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Skill name to execute" },
                    "input": { "type": "string", "description": "Input text for the skill" }
                },
                "required": ["name"]
            }),
        });

        tools
    }

    /// Check if terminal is available based on permissions.
    fn can_use_terminal(&self) -> bool {
        match self.permission {
            PermissionLevel::Normal => false,
            PermissionLevel::Chat => false,
            PermissionLevel::FullAccess => true,
            PermissionLevel::Workspace => true,
        }
    }

    /// Execute a tool call.
    pub async fn execute(&self, call: &ToolCall) -> ToolResult {
        debug!("Executing tool: {} args: {}", call.name, call.arguments);

        match call.name.as_str() {
            "terminal" => self.exec_terminal(call).await,
            "file_write" => self.exec_file_write(call).await,
            "file_read" => self.exec_file_read(call).await,
            "file_list" => self.exec_file_list(call).await,
            "file_delete" => self.exec_file_delete(call).await,
            "ask_user" => {
                // ask_user is handled by the runner (it pauses and waits for user input)
                ToolResult::ok("Waiting for user response...")
            }
            "todo_list" => {
                // todo_list is handled by the runner (it updates the UI)
                ToolResult::ok("Task list updated.")
            }
            "skill" => {
                // skill execution is handled by the skills manager
                ToolResult::err("Skill execution not yet implemented in this context")
            }
            _ => ToolResult::err(format!("Unknown tool: {}", call.name)),
        }
    }

    async fn exec_terminal(&self, call: &ToolCall) -> ToolResult {
        if !self.can_use_terminal() {
            return ToolResult::err("Terminal access not available with current permission level");
        }

        let command = call.arguments.get("command")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let timeout_secs = call.arguments.get("timeout")
            .and_then(|t| t.as_u64())
            .unwrap_or(30);

        if command.is_empty() {
            return ToolResult::err("No command provided");
        }

        info!("Terminal: {}", command);

        // Determine shell based on platform
        let (shell, flag) = if self.is_android {
            // Android: prefer Termux's bash, fallback to system sh
            let termux_bash = "/data/data/com.termux/files/usr/bin/bash";
            let bash = std::path::Path::new(termux_bash);
            if bash.exists() {
                (termux_bash, "-c")
            } else {
                ("sh", "-c")
            }
        } else if cfg!(target_os = "windows") {
            ("powershell", "-Command")
        } else {
            ("bash", "-c")
        };

        let output = tokio::task::spawn_blocking({
            let cmd = command.to_string();
            move || {
                Command::new(shell)
                    .arg(flag)
                    .arg(&cmd)
                    .output()
            }
        })
        .await;

        match output {
            Ok(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let code = out.status.code().unwrap_or(-1);

                let mut result = stdout;
                if !stderr.is_empty() {
                    if !result.is_empty() {
                        result.push_str("\n");
                    }
                    result.push_str("[stderr] ");
                    result.push_str(&stderr);
                }
                result.push_str(&format!("\n[exit code: {}]", code));

                if code == 0 {
                    ToolResult::ok(result)
                } else {
                    ToolResult {
                        success: false,
                        output: result,
                        error: Some(format!("Command exited with code {}", code)),
                        file_write_info: None,
                    }
                }
            }
            Ok(Err(e)) => ToolResult::err(format!("Failed to execute: {}", e)),
            Err(e) => ToolResult::err(format!("Task spawn failed: {}", e)),
        }
    }

    async fn exec_file_write(&self, call: &ToolCall) -> ToolResult {
        let path = call.arguments.get("path")
            .and_then(|p| p.as_str())
            .unwrap_or("");
        let content = call.arguments.get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("");

        if path.is_empty() {
            return ToolResult::err("No file path provided");
        }

        let resolved = self.resolve_path(path);
        info!("File write: {} -> {}", path, resolved.display());

        // Create parent dirs
        if let Some(parent) = resolved.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolResult::err(format!("Failed to create directories: {}", e));
            }
        }

        let bytes = content.len();
        match std::fs::write(&resolved, content) {
            Ok(_) => ToolResult::file_write(bytes, &resolved.display().to_string()),
            Err(e) => ToolResult::err(format!("Write failed: {}", e)),
        }
    }

    async fn exec_file_read(&self, call: &ToolCall) -> ToolResult {
        let path = call.arguments.get("path")
            .and_then(|p| p.as_str())
            .unwrap_or("");

        if path.is_empty() {
            return ToolResult::err("No file path provided");
        }

        let resolved = self.resolve_path(path);
        match std::fs::read_to_string(&resolved) {
            Ok(content) => {
                let truncated = if content.len() > 32 * 1024 {
                    format!("{}...\n[truncated, total {} bytes]", &content[..32 * 1024], content.len())
                } else {
                    content
                };
                ToolResult::ok(truncated)
            }
            Err(e) => ToolResult::err(format!("Read failed: {}", e)),
        }
    }

    async fn exec_file_list(&self, call: &ToolCall) -> ToolResult {
        let path = call.arguments.get("path")
            .and_then(|p| p.as_str())
            .unwrap_or(".");

        let resolved = self.resolve_path(path);
        match std::fs::read_dir(&resolved) {
            Ok(entries) => {
                let mut items: Vec<String> = Vec::new();
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let ft = entry.file_type();
                    let prefix = if ft.map(|t| t.is_dir()).unwrap_or(false) {
                        "📁 "
                    } else {
                        "📄 "
                    };
                    items.push(format!("{}{}", prefix, name));
                }
                items.sort();
                ToolResult::ok(items.join("\n"))
            }
            Err(e) => ToolResult::err(format!("List failed: {}", e)),
        }
    }

    async fn exec_file_delete(&self, call: &ToolCall) -> ToolResult {
        let path = call.arguments.get("path")
            .and_then(|p| p.as_str())
            .unwrap_or("");

        if path.is_empty() {
            return ToolResult::err("No file path provided");
        }

        let resolved = self.resolve_path(path);
        if resolved.is_dir() {
            match std::fs::remove_dir_all(&resolved) {
                Ok(_) => ToolResult::ok(format!("Deleted directory: {}", resolved.display())),
                Err(e) => ToolResult::err(format!("Delete failed: {}", e)),
            }
        } else {
            match std::fs::remove_file(&resolved) {
                Ok(_) => ToolResult::ok(format!("Deleted file: {}", resolved.display())),
                Err(e) => ToolResult::err(format!("Delete failed: {}", e)),
            }
        }
    }

    /// Resolve a path relative to workspace (if in workspace mode) or absolute.
    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            return p;
        }
        if let Some(ws) = &self.workspace {
            return ws.join(&p);
        }
        // Default to current dir
        p
    }
}