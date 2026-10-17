//! AGN global configuration types.
//! Covers: general settings, agent settings, gateway config, server config, skills.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// =========================================================================
// Theme & General Settings
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Oled,
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

/// Permission level for the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    /// Chat only, no tool execution.
    Normal,
    /// Full access — agent can execute tools, terminal, files.
    FullAccess,
    /// Workspace only — tools restricted to a workspace directory.
    Workspace,
    /// Chat with full access (same as full_access but explicitly labeled)
    Chat,
}

impl Default for PermissionLevel {
    fn default() -> Self {
        PermissionLevel::FullAccess
    }
}

/// Android-specific permission tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AndroidPermission {
    /// Normal: chat only.
    Normal,
    /// ADB Debugger (Shizuku): can use terminal, execute Android-specific commands.
    AdbDebugger,
    /// Device Admin (Dhizuku): higher than Shizuku.
    DeviceAdmin,
    /// Root: highest permission.
    Root,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    #[serde(default)]
    pub theme: Theme,
    /// Default permission level (always FullAccess — the old "full access toggle" is removed).
    #[serde(default)]
    pub permission: PermissionLevel,
    /// Desktop: Enter to send (busy = queue), Ctrl/Shift+Enter for newline.
    /// Mobile: Enter = newline, tap button to send.
    #[serde(default = "default_true")]
    pub enter_to_send: bool,
    /// Show token statistics under each AI message.
    #[serde(default = "default_true")]
    pub show_token_stats: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            permission: PermissionLevel::FullAccess,
            enter_to_send: true,
            show_token_stats: true,
        }
    }
}

fn default_true() -> bool {
    true
}

// =========================================================================
// Agent Settings
// =========================================================================

/// A character/role with its own system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    #[serde(default = "crate::new_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub system_prompt: String,
    /// Whether this character is currently active.
    #[serde(default)]
    pub active: bool,
}

/// Model parameter configuration (per-model override).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParams {
    pub model_id: String,
    #[serde(default = "default_context_length")]
    pub context_length: u32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default)]
    pub top_p: Option<f64>,
    /// Thinking depth (0=none, 1=light, 2=standard, 3=deep).
    #[serde(default)]
    pub thinking_depth: u8,
}

fn default_context_length() -> u32 {
    32768
}
fn default_max_tokens() -> u32 {
    4096
}
fn default_temperature() -> f64 {
    1.0
}

/// Auto-compression settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCompressSettings {
    /// Enable auto-compression of context.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Token threshold to trigger compression.
    #[serde(default = "default_compress_threshold")]
    pub threshold: u32,
    /// Keep last N messages uncompressed.
    #[serde(default = "default_compress_keep")]
    pub keep_recent: u32,
}

fn default_compress_threshold() -> u32 {
    24000
}
fn default_compress_keep() -> u32 {
    6
}

impl Default for AutoCompressSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 24000,
            keep_recent: 6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    /// Configured characters/roles.
    #[serde(default)]
    pub characters: Vec<Character>,
    /// Global system prompt (used when no character is active).
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    /// Per-model parameter overrides.
    #[serde(default)]
    pub model_params: Vec<ModelParams>,
    /// Global context length (used when no model-specific override exists).
    #[serde(default = "default_context_length")]
    pub global_context_length: u32,
    /// Auto-compression settings.
    #[serde(default)]
    pub auto_compress: AutoCompressSettings,
    /// Currently selected model ID.
    #[serde(default)]
    pub selected_model: String,
    /// Currently selected character ID (empty = default).
    #[serde(default)]
    pub selected_character: String,
}

fn default_system_prompt() -> String {
    "You are a helpful AI assistant. You can use tools to help the user.".to_string()
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            characters: Vec::new(),
            system_prompt: default_system_prompt(),
            model_params: Vec::new(),
            global_context_length: 32768,
            auto_compress: AutoCompressSettings::default(),
            selected_model: String::new(),
            selected_character: String::new(),
        }
    }
}

// =========================================================================
// Server Settings
// =========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    /// Whether to start the web UI.
    #[serde(default = "default_true")]
    pub start_webui: bool,
    /// Whether to start the local server.
    #[serde(default = "default_true")]
    pub start_local: bool,
    /// API server port.
    #[serde(default = "default_api_port")]
    pub api_port: u16,
    /// WebUI port.
    #[serde(default = "default_webui_port")]
    pub webui_port: u16,
    /// Remote server address (when start_local = false).
    #[serde(default)]
    pub remote_address: String,
}

fn default_api_port() -> u16 {
    11828
}
fn default_webui_port() -> u16 {
    11829
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            start_webui: true,
            start_local: true,
            api_port: 11828,
            webui_port: 11829,
            remote_address: String::new(),
        }
    }
}

// =========================================================================
// Top-level Config
// =========================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgnConfig {
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub agent: AgentSettings,
    #[serde(default)]
    pub server: ServerSettings,
    /// Gateway configuration (embedded, not a separate process).
    #[serde(default)]
    pub gateway: super::config::GatewayConfig,
}

impl AgnConfig {
    /// Load config from a file path. Returns default if file doesn't exist.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            let cfg = Self::default();
            cfg.save(path)?;
            return Ok(cfg);
        }
        let data = std::fs::read_to_string(path)?;
        let cfg: Self = serde_json::from_str(&data)?;
        Ok(cfg)
    }

    /// Save config to a file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Get the config file path in the data directory.
    pub fn config_path() -> PathBuf {
        crate::ensure_data_dir().join("config.json")
    }
}

// =========================================================================
// Gateway Config (embedded — mirrors sakura gateway.json structure)
// =========================================================================

/// Wire protocol for a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Openai,
    Anthropic,
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol::Openai
    }
}

impl Protocol {
    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("anthropic") {
            Protocol::Anthropic
        } else {
            Protocol::Openai
        }
    }
}

/// A single API key with rotation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    #[serde(default = "crate::new_id")]
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub label: String,
    /// Per-key cooldown override (seconds). None = use global default.
    #[serde(default)]
    pub cooldown_secs: Option<u64>,
    /// Cooldown learned by probing (machine-written).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_cooldown: Option<u64>,
}

/// A managed model entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedModel {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub supports_images: bool,
}

/// A provider with its key pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    #[serde(default = "crate::new_id")]
    pub id: String,
    pub name: String,
    /// Base URL, e.g. `https://api.openai.com/v1`
    pub base_url: String,
    #[serde(default)]
    pub keys: Vec<ApiKey>,
    #[serde(default)]
    pub models: Vec<ManagedModel>,
    #[serde(default)]
    pub model_allowlist_only: bool,
    #[serde(default)]
    pub protocol: Protocol,
    #[serde(default)]
    pub aliases: std::collections::BTreeMap<String, String>,
}

/// Gateway-level auth config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayAuth {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub keys: Vec<String>,
}

/// Gateway configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub auth: GatewayAuth,
    /// Default cooldown (seconds) for rate-limited keys.
    #[serde(default = "default_cooldown")]
    pub default_cooldown_secs: u64,
    /// Cooldown for "token plan exhausted" keys.
    #[serde(default = "default_token_plan_cooldown")]
    pub token_plan_cooldown_secs: u64,
    /// Cooldown for RPM/TPM exhausted keys.
    #[serde(default = "default_rpm_cooldown")]
    pub rpm_cooldown_secs: u64,
    /// Quarantine time for invalid (401) keys (seconds).
    #[serde(default = "default_quarantine")]
    pub quarantine_secs: u64,
    /// Max network retries per key.
    #[serde(default = "default_net_retries")]
    pub max_net_retries: u32,
    /// Max full rotation rounds before giving up.
    #[serde(default = "default_max_rounds")]
    pub max_rounds: u32,
}

fn default_cooldown() -> u64 {
    60
}
fn default_token_plan_cooldown() -> u64 {
    60
}
fn default_rpm_cooldown() -> u64 {
    15
}
fn default_quarantine() -> u64 {
    1800 // 30 minutes
}
fn default_net_retries() -> u32 {
    3
}
fn default_max_rounds() -> u32 {
    5
}

impl GatewayConfig {
    /// Load from a gateway.json file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read_to_string(path)?;
        let cfg: Self = serde_json::from_str(&data)?;
        Ok(cfg)
    }

    /// Save to a gateway.json file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Import a sakura gateway.json (compatible format).
    pub fn import_sakura(path: &Path) -> anyhow::Result<Self> {
        Self::load(path)
    }
}