//! AGN Gateway - Embedded LLM API gateway.
//!
//! Features:
//! - API key pool with automatic rotation
//! - Smart cooldowns: token plan exhausted = 60s, RPM/TPM = 15s, 401 = skip permanently,
//!   403/network = retry 3 times then skip
//! - Dual-protocol (OpenAI/Anthropic) bidirectional translation
//! - Streaming SSE support
//! - Key status tracking and display

pub mod protocol;
pub mod pool;
pub mod proxy;
pub mod state;

pub use protocol::Protocol;
pub use pool::{KeyPool, KeyState, PickResult};
pub use state::GatewayState;