//! Error types for AGN.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgnError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Gateway error: {0}")]
    Gateway(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {status} {message}")]
    Http { status: u16, message: String },

    #[error("All API keys exhausted or invalid")]
    AllKeysExhausted,

    #[error("Task cancelled by user")]
    TaskCancelled,

    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    #[error("Permission denied: requires {0}")]
    PermissionDenied(String),

    #[error("{0}")]
    Other(String),
}

// Note: reqwest::Error conversion is handled in agn-gateway, not here,
// to keep agn-core free of heavy HTTP client dependencies.

pub type AgnResult<T> = std::result::Result<T, AgnError>;