//! Context injection: system prompt + character prompt + tool prompt + skills.
//!
//! Injection order: system_prompt + character_prompt + tool_prompt + enabled_skills

use agn_core::{AgentSettings, Character, PermissionLevel};
use crate::tools::ToolExecutor;

/// Build the full system message injected into the conversation.
pub fn build_system_message(
    settings: &AgentSettings,
    character: Option<&Character>,
    tool_executor: &ToolExecutor,
    enabled_skills: &[SkillInfo],
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // 1. Base system prompt
    parts.push(settings.system_prompt.clone());

    // 2. Character prompt (if a character is selected)
    if let Some(char) = character {
        if !char.system_prompt.is_empty() {
            parts.push(format!("# Character: {}\n{}", char.name, char.system_prompt));
        }
    }

    // 3. Tool calling prompt
    let tool_prompt = build_tool_prompt(tool_executor);
    if !tool_prompt.is_empty() {
        parts.push(tool_prompt);
    }

    // 4. Enabled skills
    if !enabled_skills.is_empty() {
        let skills_text = enabled_skills
            .iter()
            .map(|s| format!("- **{}**: {}", s.name, s.description))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!("# Available Skills\nYou have access to the following skills. Use the `skill` tool to invoke them:\n{}", skills_text));
    }

    parts.join("\n\n---\n\n")
}

/// Build the tool usage instructions.
fn build_tool_prompt(executor: &ToolExecutor) -> String {
    let tools = executor.tool_definitions();
    if tools.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "# Tool Calling".to_string(),
        "".to_string(),
        "You have access to the following tools. Call them by name with the appropriate arguments.".to_string(),
        "".to_string(),
    ];

    for tool in &tools {
        lines.push(format!("## {}", tool.name));
        lines.push(tool.description.clone());
        if let Some(props) = tool.parameters.get("properties").and_then(|p| p.as_object()) {
            let required: Vec<&str> = tool.parameters
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            for (name, schema) in props {
                let req = if required.contains(&name.as_str()) {
                    " (required)"
                } else {
                    " (optional)"
                };
                let desc = schema.get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                let ty = schema.get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("any");
                lines.push(format!("- **{}** ({}){}: {}", name, ty, req, desc));
            }
        }
        lines.push(String::new());
    }

    // Platform-specific instructions
    if executor.is_android {
        lines.push("## Platform: Android".to_string());
        match executor.permission {
            PermissionLevel::Normal => {
                lines.push("You are running in normal mode. Terminal and file operations are limited.".to_string());
            }
            PermissionLevel::FullAccess => {
                lines.push("You have full access. You can execute terminal commands via sh, write files, and use Android-specific commands.".to_string());
            }
            PermissionLevel::Workspace => {
                if let Some(ws) = &executor.workspace {
                    lines.push(format!("You are restricted to the workspace directory: {}. File operations are limited to this path.", ws.display()));
                }
            }
            PermissionLevel::Chat => {
                lines.push("You have chat + full access mode.".to_string());
            }
        }
    } else if cfg!(target_os = "windows") {
        lines.push("## Platform: Windows".to_string());
        lines.push("Terminal commands use PowerShell. Use Windows-style paths.".to_string());
    } else {
        lines.push("## Platform: Linux".to_string());
        lines.push("Terminal commands use bash. Use Unix-style paths.".to_string());
    }

    lines.join("\n")
}

/// Skill metadata for prompt injection.
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
}