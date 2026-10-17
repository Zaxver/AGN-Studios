//! Proxy handler — forwards requests to upstream providers with key rotation.
//!
//! Implements the retry loop:
//! 1. Pick a key from the pool
//! 2. If all cooling → wait for one to recover
//! 3. If all exhausted → return error
//! 4. Send request to upstream
//! 5. On 429 → classify, apply cooldown, rotate to next key
//! 6. On 401 → permanently invalidate, rotate
//! 7. On 403/network → retry same key up to 3 times, then skip
//! 8. Max 5 full rotation rounds → give up

use crate::pool::{KeyPool, PickResult};
use crate::protocol::Protocol;
use crate::state::GatewayState;
use agn_core::RetryStatus;
use crate::protocol::RateLimitType;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use futures_util::StreamExt;
use serde_json::Value;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Gateway-level auth check (optional).
fn check_auth(state: &GatewayState, headers: &HeaderMap) -> Result<(), Response> {
    let cfg = state.read_config();
    if !cfg.auth.enabled {
        return Ok(());
    }
    let token = extract_auth_token(headers);
    if !token.is_empty() && cfg.auth.keys.iter().any(|k| k == &token) {
        return Ok(());
    }
    let body = serde_json::json!({
        "error": { "message": "invalid or missing gateway API key", "type": "gateway_error", "code": 401 }
    });
    Err((StatusCode::UNAUTHORIZED, axum::Json(body)).into_response())
}

fn extract_auth_token(headers: &HeaderMap) -> String {
    if let Some(v) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = v.to_str() {
            if let Some(t) = s.strip_prefix("Bearer ") {
                return t.to_string();
            }
        }
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

/// POST /v1/chat/completions — OpenAI-compatible proxy.
pub async fn chat_completions(
    State(state): State<std::sync::Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::Json<Value>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers) {
        return resp;
    }
    let req = body.0;
    let is_stream = req.get("stream").and_then(|s| s.as_bool()).unwrap_or(false);
    let model = req.get("model").and_then(|m| m.as_str()).unwrap_or("").to_string();

    // Resolve model alias -> upstream model name
    let cfg = state.read_config();
    let (resolved_model, provider_id_hint) = resolve_model(&cfg, &model);
    drop(cfg);

    let mut req = req;
    if resolved_model != model {
        req["model"] = Value::String(resolved_model.clone());
    }

    proxy_request(
        &state,
        &req,
        &resolved_model,
        provider_id_hint.as_deref(),
        Protocol::OpenAi,
        is_stream,
        &headers,
    )
    .await
}

/// POST /v1/messages — Anthropic-compatible proxy.
pub async fn anthropic_messages(
    State(state): State<std::sync::Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::Json<Value>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers) {
        return resp;
    }
    let anthropic_req = body.0;
    let is_stream = anthropic_req.get("stream").and_then(|s| s.as_bool()).unwrap_or(false);
    let model = anthropic_req.get("model").and_then(|m| m.as_str()).unwrap_or("").to_string();

    // Translate to OpenAI format for internal processing
    let openai_req = crate::protocol::anthropic_to_openai_request(&anthropic_req);

    proxy_request(
        &state,
        &openai_req,
        &model,
        None,
        Protocol::Anthropic,
        is_stream,
        &headers,
    )
    .await
}

/// GET /v1/models — list models from all providers.
pub async fn list_models(State(state): State<std::sync::Arc<GatewayState>>) -> Response {
    let cfg = state.read_config();
    let models: Vec<Value> = cfg
        .providers
        .iter()
        .flat_map(|p| {
            p.models.iter().filter(|m| m.enabled).map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "object": "model",
                    "owned_by": p.name,
                    "supports_images": m.supports_images,
                })
            })
        })
        .collect();

    let body = serde_json::json!({ "object": "list", "data": models });
    axum::Json(body).into_response()
}

/// Core proxy logic with key rotation and retry loop.
async fn proxy_request(
    state: &GatewayState,
    req: &Value,
    model: &str,
    provider_hint: Option<&str>,
    _inbound: Protocol,
    is_stream: bool,
    headers: &HeaderMap,
) -> Response {
    let cfg = state.read_config();
    let max_rounds = cfg.max_rounds;
    drop(cfg);

    let (total_keys, _) = state.pool.counts();
    if total_keys == 0 {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "No API keys configured. Please add a provider with keys in the gateway settings.",
        );
    }

    let mut round = 0u32;
    let mut keys_tried = 0u32;

    loop {
        round += 1;
        if round > max_rounds {
            // Exhausted all rounds
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &format!(
                    "All API keys failed after {} rotation rounds. Check key status for details.",
                    max_rounds
                ),
            );
        }

        match state.pool.pick() {
            PickResult::AllExhausted => {
                // Check if it's because all keys are token-plan exhausted
                let statuses = state.pool.status_list();
                let all_token_exhausted = statuses.iter().all(|s| {
                    s.last_error.as_deref().map_or(false, |e| {
                        e.to_lowercase().contains("token plan") && e.to_lowercase().contains("exhausted")
                    })
                });
                if all_token_exhausted {
                    return error_response(
                        StatusCode::TOO_MANY_REQUESTS,
                        "All keys token quota exhausted.",
                    );
                }
                return error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "All API keys are exhausted or invalid.",
                );
            }
            PickResult::AllCooling { wait_secs } => {
                // Wait for a key to recover
                debug!("All keys cooling, waiting {}s...", wait_secs);
                let _ = state.pool.wait_for_available(wait_secs.max(5)).await;
                continue;
            }
            PickResult::Picked {
                provider_id,
                provider_name,
                base_url,
                protocol,
                key_id,
                key_secret,
            } => {
                keys_tried += 1;

                // Build upstream URL
                let upstream_url = match protocol {
                    agn_core::Protocol::Openai => format!("{}/chat/completions", base_url.trim_end_matches('/')),
                    agn_core::Protocol::Anthropic => format!("{}/messages", base_url.trim_end_matches('/')),
                };

                // Translate request if upstream is Anthropic but we have OpenAI
                let upstream_req = match protocol {
                    agn_core::Protocol::Anthropic => {
                        crate::protocol::openai_to_anthropic_request(req)
                    }
                    _ => req.clone(),
                };

                // Build the HTTP request
                let mut req_builder = state.http
                    .post(&upstream_url)
                    .header("Content-Type", "application/json")
                    .timeout(std::time::Duration::from_secs(300));

                // Auth header based on protocol
                match protocol {
                    agn_core::Protocol::Openai => {
                        req_builder = req_builder.bearer_auth(&key_secret);
                    }
                    agn_core::Protocol::Anthropic => {
                        req_builder = req_builder
                            .header("x-api-key", &key_secret)
                            .header("anthropic-version", "2023-06-01");
                    }
                }

                // Forward select headers
                if let Some(idempotency) = headers.get("x-idempotency-key") {
                    req_builder = req_builder.header("x-idempotency-key", idempotency);
                }

                let req_start = Instant::now();
                let send_result = req_builder.json(&upstream_req).send().await;

                match send_result {
                    Ok(resp) => {
                        let status = resp.status();

                        if status == StatusCode::TOO_MANY_REQUESTS {
                            // 429 — classify and apply cooldown
                            let body_text = resp.text().await.unwrap_or_default();
                            let rl = state.pool.record_429(&key_id, &body_text);

                            info!(
                                "Key {} (provider {}) got 429 {:?}, rotating",
                                key_id, provider_name, rl
                            );

                            // Continue to next key
                            continue;
                        }

                        if status == StatusCode::UNAUTHORIZED {
                            // 401 — permanently invalidate
                            state.pool.record_401(&key_id, "Unauthorized");
                            warn!("Key {} (provider {}) got 401, invalidating", key_id, provider_name);
                            continue;
                        }

                        if status == StatusCode::FORBIDDEN || status.is_server_error() {
                            // 403 or 5xx — treat as network error, retry
                            let should_retry = state.pool.record_network_error(
                                &key_id,
                                &format!("{}: {}", status, provider_name),
                            );
                            if should_retry {
                                continue;
                            }
                            continue;
                        }

                        if status.is_success() {
                            // Success!
                            state.pool.record_success(&key_id);

                            // Extract usage stats from response
                            let elapsed = req_start.elapsed();

                            if is_stream {
                                // Stream the response back
                                return stream_response(resp, model, &provider_name, &key_id, elapsed).await;
                            } else {
                                // Non-streaming: parse, extract usage, forward
                                let body_bytes = resp.bytes().await.unwrap_or_default();
                                let body_str = String::from_utf8_lossy(&body_bytes);
                                let body_val: Value = serde_json::from_str(&body_str).unwrap_or(Value::Null);

                                // Extract usage
                                if let Some(usage) = extract_usage(&body_val) {
                                    state.stats.record(model, &provider_name, &usage, true);
                                }

                                return Response::builder()
                                    .status(StatusCode::OK)
                                    .header("Content-Type", "application/json")
                                    .body(Body::from(body_bytes))
                                    .unwrap_or_else(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to build response"));
                            }
                        }

                        // Other error status
                        let body_text = resp.text().await.unwrap_or_default();
                        warn!("Key {} got unexpected status {}: {}", key_id, status, body_text.chars().take(200).collect::<String>());
                        let should_retry = state.pool.record_network_error(&key_id, &format!("{}: {}", status, body_text));
                        if should_retry {
                            continue;
                        }
                        continue;
                    }
                    Err(e) => {
                        // Network error
                        let should_retry = state.pool.record_network_error(&key_id, &format!("Network error: {}", e));
                        if !should_retry {
                            warn!("Key {} network-skipped: {}", key_id, e);
                        }
                        continue;
                    }
                }
            }
        }
    }
}

/// Resolve a model name to its upstream equivalent and hint which provider to use.
fn resolve_model(cfg: &agn_core::GatewayConfig, model: &str) -> (String, Option<String>) {
    // Check provider aliases first
    for p in &cfg.providers {
        if let Some(upstream) = p.aliases.get(model) {
            return (upstream.clone(), Some(p.id.clone()));
        }
    }
    // Check if the model is a "provider/model" format
    if let Some((prov_name, model_name)) = model.split_once('/') {
        for p in &cfg.providers {
            if p.name.eq_ignore_ascii_case(prov_name) {
                return (model_name.to_string(), Some(p.id.clone()));
            }
        }
    }
    // Default: model name as-is, no provider hint
    (model.to_string(), None)
}

/// Extract token usage from an OpenAI response.
fn extract_usage(body: &Value) -> Option<agn_core::TokenUsage> {
    let u = body.get("usage")?;
    Some(agn_core::TokenUsage {
        prompt_tokens: u.get("prompt_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        completion_tokens: u.get("completion_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        total_tokens: u.get("total_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        time_to_first_token_ms: None,
        tokens_per_second: None,
    })
}

/// Stream an SSE response back to the client, extracting usage from the final chunk.
async fn stream_response(
    resp: reqwest::Response,
    _model: &str,
    provider_name: &str,
    _key_id: &str,
    _elapsed: std::time::Duration,
) -> Response {
    let stream = resp.bytes_stream().map(|chunk| {
        chunk.map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })
    });

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Stream failed"))
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({
        "error": { "message": message, "type": "gateway_error", "code": status.as_u16() }
    });
    (status, axum::Json(body)).into_response()
}

use axum::response::IntoResponse;