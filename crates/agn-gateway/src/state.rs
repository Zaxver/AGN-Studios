//! Gateway shared state — holds the key pool, config, and HTTP client.

use agn_core::{GatewayConfig, StatsTracker};
use std::sync::Arc;
use std::sync::RwLock;

use crate::pool::KeyPool;

/// The gateway's shared state, passed to all route handlers.
/// All fields use interior mutability (Mutex/RwLock), so the struct itself
/// can be shared safely. Axum requires State to be Clone, so we use Arc.
pub struct GatewayState {
    pub pool: KeyPool,
    pub config: RwLock<GatewayConfig>,
    pub http: reqwest::Client,
    pub stats: StatsTracker,
    pub config_path: std::path::PathBuf,
}

impl GatewayState {
    pub fn new(config: GatewayConfig, config_path: std::path::PathBuf) -> Self {
        let pool = KeyPool::new(config.clone());
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            pool,
            config: RwLock::new(config),
            http,
            stats: StatsTracker::new(),
            config_path,
        }
    }

    pub fn read_config(&self) -> GatewayConfig {
        self.config.read().unwrap().clone()
    }

    pub fn reload_config(&self, config: GatewayConfig) {
        self.pool.reload(config.clone());
        *self.config.write().unwrap() = config;
    }

    pub fn save_config(&self) -> anyhow::Result<()> {
        let cfg = self.read_config();
        cfg.save(&self.config_path)?;
        Ok(())
    }
}

pub type SharedGatewayState = Arc<GatewayState>;