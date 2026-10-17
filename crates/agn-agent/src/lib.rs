//! AGN Agent - AI agent with tool calling, terminal execution, and skills.
//!
//! Features:
//! - Tool calling: terminal, file operations, skill execution, ask user, todo list
//! - Platform-aware: Win/Linux uses terminal/powershell, Android uses Termux
//! - Permission-based tool availability (normal, adbdebugger, deviceadmin, root)
//! - Context injection: system prompt + character prompt + tool prompt + skills

pub mod tools;
pub mod prompt;
pub mod runner;
pub mod skills;

pub use tools::{ToolExecutor, ToolResult};
pub use runner::AgentRunner;