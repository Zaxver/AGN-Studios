//! Admin API routes and WebSocket handler.

use crate::SharedState;
use agn_core::*;
use axum::extract::{Path, State, Query, WebSocketUpgrade, ws::Message as WsMessage};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put, delete};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

/// Build the admin API router.
pub fn admin_router() -> Router<SharedState> {
    Router::new()
        // Conversations
        .route("/api/conversations", get(list_conversations).post(create_conversation))
        .route("/api/conversations/{id}", get(get_conversation).put(update_conversation).delete(delete_conversation))
        .route("/api/conversations/{id}/messages", post(send_message))
        .route("/api/conversations/{id}/messages/{msg_id}", delete(delete_message))
        .route("/api/conversations/{id}/group", put(update_group))
        // Config
        .route("/api/config", get(get_config).put(update_config))
        // Gateway
        .route("/api/gateway", get(get_gateway).put(update_gateway))
        .route("/api/gateway/providers", get(list_providers).post(add_provider))
        .route("/api/gateway/providers/{id}", put(update_provider).delete(delete_provider))
        .route("/api/gateway/providers/{id}/keys", post(add_keys))
        .route("/api/gateway/import", post(import_gateway))
        .route("/api/gateway/status", get(key_status))
        // Skills
        .route("/api/skills", get(list_skills).post(install_skill))
        .route("/api/skills/{name}", delete(remove_skill).put(toggle_skill))
        // Stats
        .route("/api/stats", get(get_stats))
        // Models
        .route("/api/models", get(list_models))
}

// === Conversations ===

async fn list_conversations(State(state): State<SharedState>) -> Response {
    let convs = state.conversations.read().unwrap();
    Json(convs.clone()).into_response()
}

async fn create_conversation(State(state): State<SharedState>, Json(body): Json<Value>) -> Response {
    let title = body.get("title").and_then(|t| t.as_str()).unwrap_or("New Chat");
    let conv = Conversation::new(title);
    let mut convs = state.conversations.write().unwrap();
    convs.push(conv.clone());
    Json(conv).into_response()
}

async fn get_conversation(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let convs = state.conversations.read().unwrap();
    match convs.iter().find(|c| c.id == id) {
        Some(c) => Json(c.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "Conversation not found").into_response(),
    }
}

async fn update_conversation(State(state): State<SharedState>, Path(id): Path<String>, Json(body): Json<Conversation>) -> Response {
    let mut convs = state.conversations.write().unwrap();
    if let Some(c) = convs.iter_mut().find(|c| c.id == id) {
        *c = body;
        return Json(c.clone()).into_response();
    }
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

async fn delete_conversation(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let mut convs = state.conversations.write().unwrap();
    convs.retain(|c| c.id != id);
    StatusCode::OK.into_response()
}

async fn send_message(State(state): State<SharedState>, Path(id): Path<String>, Json(body): Json<Value>) -> Response {
    let content = body.get("content").and_then(|c| c.as_str()).unwrap_or("");
    if content.is_empty() {
        return (StatusCode::BAD_REQUEST, "Empty message").into_response();
    }

    let mut convs = state.conversations.write().unwrap();
    let conv = match convs.iter_mut().find(|c| c.id == id) {
        Some(c) => c,
        None => return (StatusCode::NOT_FOUND, "Conversation not found").into_response(),
    };

    // Add user message
    conv.messages.push(ChatMessage::user(content));
    conv.updated_at = chrono::Utc::now().to_rfc3339();

    // For now, return the updated conversation. The actual generation
    // happens via WebSocket for streaming.
    Json(conv.clone()).into_response()
}

async fn delete_message(State(state): State<SharedState>, Path((conv_id, msg_id)): Path<(String, String)>) -> Response {
    let mut convs = state.conversations.write().unwrap();
    if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
        conv.messages.retain(|m| m.id != msg_id);
        return StatusCode::OK.into_response();
    }
    (StatusCode::NOT_FOUND, "Conversation not found").into_response()
}

async fn update_group(State(state): State<SharedState>, Path(id): Path<String>, Json(body): Json<Value>) -> Response {
    let group = body.get("group").and_then(|g| g.as_str()).unwrap_or("");
    let mut convs = state.conversations.write().unwrap();
    if let Some(conv) = convs.iter_mut().find(|c| c.id == id) {
        conv.group = group.to_string();
        return Json(conv.clone()).into_response();
    }
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

// === Config ===

async fn get_config(State(state): State<SharedState>) -> Response {
    let cfg = state.config.read().unwrap();
    Json(cfg.clone()).into_response()
}

async fn update_config(State(state): State<SharedState>, Json(cfg): Json<AgnConfig>) -> Response {
    let mut config = state.config.write().unwrap();
    *config = cfg;
    // Save to disk
    let path = AgnConfig::config_path();
    let _ = config.save(&path);
    Json(config.clone()).into_response()
}

// === Gateway ===

async fn get_gateway(State(state): State<SharedState>) -> Response {
    let cfg = state.gateway.read_config();
    Json(cfg).into_response()
}

async fn update_gateway(State(state): State<SharedState>, Json(cfg): Json<GatewayConfig>) -> Response {
    state.gateway.reload_config(cfg.clone());
    let _ = state.gateway.save_config();
    Json(cfg).into_response()
}

async fn list_providers(State(state): State<SharedState>) -> Response {
    let cfg = state.gateway.read_config();
    Json(&cfg.providers).into_response()
}

#[derive(Deserialize)]
struct ProviderInput {
    name: String,
    base_url: String,
    protocol: Option<String>,
}

async fn add_provider(State(state): State<SharedState>, Json(body): Json<ProviderInput>) -> Response {
    let mut cfg = state.gateway.read_config();
    let provider = Provider {
        id: agn_core::new_id(),
        name: body.name,
        base_url: body.base_url,
        keys: Vec::new(),
        models: Vec::new(),
        model_allowlist_only: false,
        protocol: body.protocol.as_deref().map(Protocol::parse).unwrap_or_default(),
        aliases: Default::default(),
    };
    cfg.providers.push(provider.clone());
    state.gateway.reload_config(cfg.clone());
    let _ = state.gateway.save_config();
    Json(provider).into_response()
}

async fn update_provider(State(state): State<SharedState>, Path(id): Path<String>, Json(body): Json<Provider>) -> Response {
    let mut cfg = state.gateway.read_config();
    if let Some(p) = cfg.providers.iter_mut().find(|p| p.id == id) {
        *p = body;
        let updated = p.clone();
        state.gateway.reload_config(cfg);
        let _ = state.gateway.save_config();
        return Json(updated).into_response();
    }
    (StatusCode::NOT_FOUND, "Provider not found").into_response()
}

async fn delete_provider(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let mut cfg = state.gateway.read_config();
    cfg.providers.retain(|p| p.id != id);
    state.gateway.reload_config(cfg);
    let _ = state.gateway.save_config();
    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
struct AddKeysInput {
    keys: String, // newline-separated keys
}

async fn add_keys(State(state): State<SharedState>, Path(provider_id): Path<String>, Json(body): Json<AddKeysInput>) -> Response {
    let mut cfg = state.gateway.read_config();
    if let Some(p) = cfg.providers.iter_mut().find(|p| p.id == provider_id) {
        for line in body.keys.lines() {
            let key = line.trim();
            if !key.is_empty() {
                p.keys.push(ApiKey {
                    id: agn_core::new_id(),
                    key: key.to_string(),
                    label: String::new(),
                    cooldown_secs: None,
                    learned_cooldown: None,
                });
            }
        }
        let keys_count = p.keys.len();
        state.gateway.reload_config(cfg);
        let _ = state.gateway.save_config();
        return Json(serde_json::json!({"keys_added": keys_count})).into_response();
    }
    (StatusCode::NOT_FOUND, "Provider not found").into_response()
}

async fn import_gateway(State(state): State<SharedState>, Json(body): Json<Value>) -> Response {
    let path = body.get("path").and_then(|p| p.as_str());
    if let Some(path) = path {
        match GatewayConfig::load(std::path::Path::new(path)) {
            Ok(cfg) => {
                state.gateway.reload_config(cfg.clone());
                let _ = state.gateway.save_config();
                return Json(serde_json::json!({"success": true, "providers": cfg.providers.len()})).into_response();
            }
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("Import failed: {}", e)).into_response();
            }
        }
    }
    // Try inline import
    if let Some(cfg) = body.get("config") {
        match serde_json::from_value::<GatewayConfig>(cfg.clone()) {
            Ok(cfg) => {
                state.gateway.reload_config(cfg.clone());
                let _ = state.gateway.save_config();
                return Json(serde_json::json!({"success": true, "providers": cfg.providers.len()})).into_response();
            }
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("Parse failed: {}", e)).into_response();
            }
        }
    }
    (StatusCode::BAD_REQUEST, "No path or config provided").into_response()
}

async fn key_status(State(state): State<SharedState>) -> Response {
    let statuses = state.gateway.pool.status_list();
    Json(statuses).into_response()
}

// === Skills ===

async fn list_skills(State(state): State<SharedState>) -> Response {
    let skills = state.skills.list();
    Json(skills).into_response()
}

#[derive(Deserialize)]
struct InstallSkillInput {
    url: Option<String>,
    path: Option<String>,
}

async fn install_skill(State(state): State<SharedState>, Json(body): Json<InstallSkillInput>) -> Response {
    if let Some(url) = &body.url {
        match state.skills.install_from_github(url).await {
            Ok(name) => return Json(serde_json::json!({"success": true, "name": name})).into_response(),
            Err(e) => return (StatusCode::BAD_REQUEST, format!("Install failed: {}", e)).into_response(),
        }
    }
    if let Some(path) = &body.path {
        match state.skills.install_from_local(std::path::Path::new(path)) {
            Ok(name) => return Json(serde_json::json!({"success": true, "name": name})).into_response(),
            Err(e) => return (StatusCode::BAD_REQUEST, format!("Install failed: {}", e)).into_response(),
        }
    }
    (StatusCode::BAD_REQUEST, "No URL or path provided").into_response()
}

async fn remove_skill(State(state): State<SharedState>, Path(name): Path<String>) -> Response {
    match state.skills.remove(&name) {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

async fn toggle_skill(State(state): State<SharedState>, Path(name): Path<String>, Json(body): Json<Value>) -> Response {
    let enabled = body.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true);
    match state.skills.toggle(&name, enabled) {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

// === Stats ===

async fn get_stats(State(state): State<SharedState>) -> Response {
    let stats = state.gateway.stats.snapshot();
    Json(stats).into_response()
}

// === Models ===

async fn list_models(State(state): State<SharedState>) -> Response {
    let cfg = state.gateway.read_config();
    let models: Vec<Value> = cfg.providers.iter().flat_map(|p| {
        p.models.iter().filter(|m| m.enabled).map(|m| {
            serde_json::json!({
                "id": m.id,
                "provider": p.name,
                "context_length": m.context_length,
                "supports_images": m.supports_images,
            })
        })
    }).collect();
    Json(serde_json::json!({"models": models})).into_response()
}

// === WebSocket ===

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: axum::extract::ws::WebSocket, state: SharedState) {
    use futures_util::SinkExt;

    info!("WebSocket connected");

    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            WsMessage::Text(text) => {
                // Parse incoming message
                if let Ok(req) = serde_json::from_str::<Value>(&text) {
                    let msg_type = req.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    match msg_type {
                        "chat" => {
                            // Start a generation
                            let conv_id = req.get("conversation_id").and_then(|c| c.as_str()).unwrap_or("");
                            let content = req.get("content").and_then(|c| c.as_str()).unwrap_or("");

                            // Send acknowledgment
                            let _ = socket.send(WsMessage::Text(
                                serde_json::json!({"type": "status", "state": "generating", "message": "正在生成回复..."}).to_string().into()
                            )).await;

                            // For now, just echo back
                            // Full implementation would spawn an AgentRunner
                            let _ = socket.send(WsMessage::Text(
                                serde_json::json!({"type": "text_chunk", "content": format!("Echo: {}", content)}).to_string().into()
                            )).await;
                            let _ = socket.send(WsMessage::Text(
                                serde_json::json!({"type": "done"}).to_string().into()
                            )).await;
                        }
                        "ping" => {
                            let _ = socket.send(WsMessage::Text(
                                serde_json::json!({"type": "pong"}).to_string().into()
                            )).await;
                        }
                        _ => {}
                    }
                }
            }
            WsMessage::Close(_) => {
                info!("WebSocket closed");
                break;
            }
            _ => {}
        }
    }
}