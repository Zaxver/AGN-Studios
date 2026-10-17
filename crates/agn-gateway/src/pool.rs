//! Key pool with automatic rotation and smart cooldowns.
//!
//! Cooldown rules (per the spec):
//! - 429 "token plan exhausted" → cooldown 60s
//! - 429 "rpm/tpm/inference exhausted" → cooldown 15s
//! - 429 other → cooldown default (60s)
//! - 401 → permanent quarantine (skip this key forever)
//! - 403 or network error → retry this key 3 times, then skip (yellow network error)
//! - If all keys cooling → wait (block) until one recovers, don't give up
//! - Max 5 full rotation rounds across all keys before giving up on network errors

use agn_core::{GatewayConfig, KeyStatus, KeyStatusInfo};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use crate::protocol::{classify_429, cooldown_for, RateLimitType};

/// Internal state of a single API key.
#[derive(Debug, Clone)]
pub struct KeyState {
    pub provider_id: String,
    pub provider_name: String,
    pub key_id: String,
    pub key_secret: String,
    pub key_label: String,
    /// Cooldown expiry (Instant). None = not cooling.
    pub cooldown_until: Option<Instant>,
    /// Whether this key is permanently invalid (401).
    pub invalid: bool,
    /// Network error retry count (resets on success).
    pub net_retries: u32,
    /// Whether this key has been skipped due to network errors.
    pub network_skip: bool,
    /// Last error message.
    pub last_error: Option<String>,
    /// Total requests routed through this key.
    pub total_requests: u64,
    /// Whether this key has ever been used.
    pub ever_used: bool,
    /// The reason for the current cooldown (if any).
    pub cooldown_reason: Option<String>,
}

impl KeyState {
    pub fn new(
        provider_id: &str,
        provider_name: &str,
        key_id: &str,
        key_secret: &str,
        key_label: &str,
    ) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            provider_name: provider_name.to_string(),
            key_id: key_id.to_string(),
            key_secret: key_secret.to_string(),
            key_label: key_label.to_string(),
            cooldown_until: None,
            invalid: false,
            net_retries: 0,
            network_skip: false,
            last_error: None,
            total_requests: 0,
            ever_used: false,
            cooldown_reason: None,
        }
    }

    pub fn is_available(&self) -> bool {
        !self.invalid
            && self.cooldown_until.map_or(true, |t| Instant::now() >= t)
            && !self.network_skip
    }

    pub fn remaining_cooldown(&self) -> Option<u64> {
        self.cooldown_until.and_then(|t| {
            let now = Instant::now();
            if t > now {
                Some(t.duration_since(now).as_secs())
            } else {
                None
            }
        })
    }

    pub fn to_status_info(&self) -> KeyStatusInfo {
        let status = if self.invalid {
            KeyStatus::Invalid
        } else if self.network_skip {
            KeyStatus::NetworkError
        } else if self.cooldown_until.map_or(false, |t| t > Instant::now()) {
            KeyStatus::Cooling
        } else if self.ever_used {
            KeyStatus::Available
        } else {
            KeyStatus::Unused
        };

        KeyStatusInfo {
            provider_id: self.provider_id.clone(),
            provider_name: self.provider_name.clone(),
            key_id: self.key_id.clone(),
            key_label: self.key_label.clone(),
            status,
            cooldown_remaining: self.remaining_cooldown(),
            last_error: self.last_error.clone(),
        }
    }
}

/// Result of picking a key for a request.
#[derive(Debug)]
pub enum PickResult {
    /// A key was picked successfully.
    Picked {
        provider_id: String,
        provider_name: String,
        base_url: String,
        protocol: agn_core::Protocol,
        key_id: String,
        key_secret: String,
    },
    /// All keys are cooling — need to wait for one to recover.
    AllCooling {
        /// Seconds until the earliest key recovers.
        wait_secs: u64,
    },
    /// All keys are exhausted (invalid or no keys configured).
    AllExhausted,
}

/// The key pool manager.
pub struct KeyPool {
    /// All keys across all providers, flattened.
    keys: Mutex<Vec<KeyState>>,
    /// Round-robin cursor (index into keys vec).
    cursor: Mutex<usize>,
    /// Gateway config (for cooldown durations).
    config: Mutex<GatewayConfig>,
}

impl KeyPool {
    pub fn new(config: GatewayConfig) -> Self {
        let mut keys = Vec::new();
        for p in &config.providers {
            for k in &p.keys {
                keys.push(KeyState::new(
                    &p.id,
                    &p.name,
                    &k.id,
                    &k.key,
                    &k.label,
                ));
            }
        }
        Self {
            keys: Mutex::new(keys),
            cursor: Mutex::new(0),
            config: Mutex::new(config),
        }
    }

    /// Reload keys from config (hot-reload).
    pub fn reload(&self, config: GatewayConfig) {
        let mut keys = Vec::new();
        for p in &config.providers {
            for k in &p.keys {
                keys.push(KeyState::new(
                    &p.id,
                    &p.name,
                    &k.id,
                    &k.key,
                    &k.label,
                ));
            }
        }
        *self.keys.lock().unwrap() = keys;
        *self.cursor.lock().unwrap() = 0;
        *self.config.lock().unwrap() = config;
    }

    /// Pick the next available key (round-robin, skipping cooled/invalid).
    pub fn pick(&self) -> PickResult {
        let keys = self.keys.lock().unwrap();
        let cfg = self.config.lock().unwrap();
        let mut cursor = self.cursor.lock().unwrap();

        if keys.is_empty() {
            return PickResult::AllExhausted;
        }

        let n = keys.len();
        let mut all_invalid = true;
        let mut earliest_cooldown: Option<Instant> = None;
        let mut any_available = false;

        // Try round-robin from current cursor
        for i in 0..n {
            let idx = (*cursor + i) % n;
            let k = &keys[idx];

            if k.invalid {
                continue;
            }
            all_invalid = false;

            if k.network_skip {
                continue;
            }

            if let Some(cooldown_until) = k.cooldown_until {
                if Instant::now() < cooldown_until {
                    // Track earliest recovery
                    earliest_cooldown = Some(earliest_cooldown.map_or(cooldown_until, |ec| {
                        cooldown_until.min(ec)
                    }));
                    continue;
                }
            }

            // This key is available!
            *cursor = (idx + 1) % n;
            any_available = true;

            // Find the provider for this key
            if let Some(provider) = cfg.providers.iter().find(|p| p.id == k.provider_id) {
                return PickResult::Picked {
                    provider_id: provider.id.clone(),
                    provider_name: provider.name.clone(),
                    base_url: provider.base_url.clone(),
                    protocol: provider.protocol,
                    key_id: k.key_id.clone(),
                    key_secret: k.key_secret.clone(),
                };
            }
        }

        if all_invalid {
            return PickResult::AllExhausted;
        }

        if !any_available {
            // All are cooling — wait for the earliest
            if let Some(ec) = earliest_cooldown {
                let wait = ec.saturating_duration_since(Instant::now());
                let wait_secs = wait.as_secs().max(1);
                debug!("All keys cooling, waiting {}s", wait_secs);
                return PickResult::AllCooling { wait_secs };
            }
        }

        PickResult::AllExhausted
    }

    /// Record a successful request on a key.
    pub fn record_success(&self, key_id: &str) {
        let mut keys = self.keys.lock().unwrap();
        if let Some(k) = keys.iter_mut().find(|k| k.key_id == key_id) {
            k.total_requests += 1;
            k.ever_used = true;
            k.cooldown_until = None;
            k.cooldown_reason = None;
            k.net_retries = 0;
            k.network_skip = false;
            k.last_error = None;
        }
    }

    /// Record a 429 error on a key (applies appropriate cooldown).
    pub fn record_429(&self, key_id: &str, body: &str) -> RateLimitType {
        let rl = classify_429(body);
        let cfg = self.config.lock().unwrap();
        let cooldown_secs = cooldown_for(&rl, &cfg);
        drop(cfg);

        let mut keys = self.keys.lock().unwrap();
        if let Some(k) = keys.iter_mut().find(|k| k.key_id == key_id) {
            k.cooldown_until = Some(Instant::now() + Duration::from_secs(cooldown_secs));
            k.cooldown_reason = Some(format!("429 {:?}", rl));
            k.last_error = Some(format!("429: {}", body.chars().take(200).collect::<String>()));
        }
        debug!("Key {} 429 cooldown {}s ({:?})", key_id, cooldown_secs, rl);
        rl
    }

    /// Record a 401 error — permanently invalidate the key.
    pub fn record_401(&self, key_id: &str, body: &str) {
        let mut keys = self.keys.lock().unwrap();
        if let Some(k) = keys.iter_mut().find(|k| k.key_id == key_id) {
            k.invalid = true;
            k.last_error = Some(format!("401: {}", body.chars().take(200).collect::<String>()));
        }
        warn!("Key {} permanently invalidated (401)", key_id);
    }

    /// Record a 403 or network error — retry up to max_net_retries, then skip.
    /// Returns true if the key should still be retried, false if it's now skipped.
    pub fn record_network_error(&self, key_id: &str, error: &str) -> bool {
        let cfg = self.config.lock().unwrap();
        let max_retries = cfg.max_net_retries;
        drop(cfg);

        let mut keys = self.keys.lock().unwrap();
        if let Some(k) = keys.iter_mut().find(|k| k.key_id == key_id) {
            k.net_retries += 1;
            k.last_error = Some(error.to_string());

            if k.net_retries >= max_retries {
                k.network_skip = true;
                debug!("Key {} skipped after {} network retries", key_id, k.net_retries);
                return false;
            }
            debug!("Key {} network retry {}/{}", key_id, k.net_retries, max_retries);
            return true;
        }
        false
    }

    /// Check if all keys are either invalid or network-skipped (not just cooling).
    pub fn all_dead(&self) -> bool {
        let keys = self.keys.lock().unwrap();
        if keys.is_empty() {
            return true;
        }
        keys.iter().all(|k| k.invalid || k.network_skip)
    }

    /// Count total keys and available keys.
    pub fn counts(&self) -> (usize, usize) {
        let keys = self.keys.lock().unwrap();
        let total = keys.len();
        let available = keys.iter().filter(|k| k.is_available()).count();
        (total, available)
    }

    /// Get all key statuses for the status panel.
    pub fn status_list(&self) -> Vec<KeyStatusInfo> {
        let keys = self.keys.lock().unwrap();
        keys.iter().map(|k| k.to_status_info()).collect()
    }

    /// Wait for a key to become available (blocking async).
    pub async fn wait_for_available(&self, max_wait_secs: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(max_wait_secs);
        loop {
            match self.pick() {
                PickResult::Picked { .. } => return true,
                PickResult::AllCooling { wait_secs } => {
                    let wait = Duration::from_secs(wait_secs.min(5));
                    if Instant::now() + wait > deadline {
                        return false;
                    }
                    tokio::time::sleep(wait).await;
                }
                PickResult::AllExhausted => return false,
            }
        }
    }
}