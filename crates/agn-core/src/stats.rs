//! Token usage statistics tracking (cumulative).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Cumulative token statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenStats {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_tokens: u64,
    /// Total number of requests.
    pub total_requests: u64,
    /// Number of successful requests.
    pub successful_requests: u64,
    /// Number of failed requests.
    pub failed_requests: u64,
    /// Per-model statistics.
    #[serde(default)]
    pub per_model: HashMap<String, ModelStats>,
    /// Per-provider statistics.
    #[serde(default)]
    pub per_provider: HashMap<String, ProviderStats>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelStats {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub requests: u64,
    pub avg_tps: f32,
    pub avg_tftt_ms: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderStats {
    pub requests: u64,
    pub successful: u64,
    pub failed: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// Thread-safe stats tracker.
pub struct StatsTracker {
    stats: Mutex<TokenStats>,
}

impl StatsTracker {
    pub fn new() -> Self {
        Self {
            stats: Mutex::new(TokenStats::default()),
        }
    }

    pub fn record(&self, model: &str, provider: &str, usage: &super::chat::TokenUsage, success: bool) {
        let mut s = self.stats.lock().unwrap();
        s.total_prompt_tokens += usage.prompt_tokens as u64;
        s.total_completion_tokens += usage.completion_tokens as u64;
        s.total_tokens += usage.total_tokens as u64;
        s.total_requests += 1;
        if success {
            s.successful_requests += 1;
        } else {
            s.failed_requests += 1;
        }

        let m = s.per_model.entry(model.to_string()).or_default();
        m.prompt_tokens += usage.prompt_tokens as u64;
        m.completion_tokens += usage.completion_tokens as u64;
        m.total_tokens += usage.total_tokens as u64;
        m.requests += 1;

        let p = s.per_provider.entry(provider.to_string()).or_default();
        p.requests += 1;
        p.prompt_tokens += usage.prompt_tokens as u64;
        p.completion_tokens += usage.completion_tokens as u64;
        if success {
            p.successful += 1;
        } else {
            p.failed += 1;
        }
    }

    pub fn snapshot(&self) -> TokenStats {
        self.stats.lock().unwrap().clone()
    }

    /// Calculate cache hit rate (if we had caching — placeholder for now).
    pub fn hit_rate(&self) -> f64 {
        let s = self.stats.lock().unwrap();
        if s.total_requests == 0 {
            return 0.0;
        }
        // Placeholder: would track cache hits vs misses
        0.0
    }
}

impl Default for StatsTracker {
    fn default() -> Self {
        Self::new()
    }
}