//! AGN Server — HTTP API server with WebSocket, embedded gateway, and WebUI.
//!
//! Routes:
//! - Gateway: /v1/chat/completions, /v1/messages, /v1/models (proxied to upstream)
//! - Admin API: /api/conversations, /api/config, /api/gateway, /api/skills, /api/stats
//! - WebSocket: /ws (real-time chat events)
//! - WebUI: embedded static files from webui/

pub mod routes;
pub mod webui;

use agn_core::{AgnConfig, ensure_data_dir};
use agn_gateway::state::GatewayState;
use agn_agent::skills::SkillsManager;
use axum::Router;
use std::sync::Arc;
use std::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;

/// Shared application state.
pub struct AppState {
    pub config: RwLock<AgnConfig>,
    pub gateway: std::sync::Arc<agn_gateway::state::GatewayState>,
    pub skills: SkillsManager,
    pub conversations: RwLock<Vec<agn_core::Conversation>>,
}

impl AppState {
    pub fn new(config: AgnConfig) -> Self {
        let data_dir = ensure_data_dir();
        let gateway_config = config.gateway.clone();
        let gateway_path = data_dir.join("gateway.json");
        let gateway = std::sync::Arc::new(agn_gateway::state::GatewayState::new(gateway_config, gateway_path));
        let skills = SkillsManager::new(&data_dir);

        Self {
            config: RwLock::new(config),
            gateway,
            skills,
            conversations: RwLock::new(Vec::new()),
        }
    }
}

pub type SharedState = Arc<AppState>;

/// Start the AGN server.
pub async fn run_server(config: AgnConfig) -> anyhow::Result<()> {
    let state = Arc::new(AppState::new(config.clone()));
    let api_port = config.server.api_port;
    let webui_port = config.server.webui_port;

    // Gateway proxy router (uses Arc<GatewayState>, which is Clone)
    let gateway_state = state.gateway.clone();
    let gateway_router = Router::new()
        .route("/v1/chat/completions", axum::routing::post(agn_gateway::proxy::chat_completions))
        .route("/v1/messages", axum::routing::post(agn_gateway::proxy::anthropic_messages))
        .route("/v1/models", axum::routing::get(agn_gateway::proxy::list_models))
        .with_state(gateway_state);

    // Admin API + WebSocket router (uses AppState)
    let admin_router = Router::new()
        .merge(routes::admin_router())
        .route("/ws", axum::routing::get(routes::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    // Merge both routers
    let api_app = Router::new()
        .merge(gateway_router)
        .merge(admin_router);

    // Start API server
    let api_addr = format!("0.0.0.0:{}", api_port);
    info!("AGN API server starting on {}", api_addr);

    let api_listener = tokio::net::TcpListener::bind(&api_addr).await?;
    let api_handle = tokio::spawn(async move {
        axum::serve(api_listener, api_app).await.unwrap();
    });

    // Start WebUI server (if enabled)
    if config.server.start_webui {
        let webui_app = Router::new()
            .fallback(webui::serve_webui);

        let webui_addr = format!("0.0.0.0:{}", webui_port);
        info!("AGN WebUI server starting on {}", webui_addr);

        let webui_listener = tokio::net::TcpListener::bind(&webui_addr).await?;
        let webui_handle = tokio::spawn(async move {
            axum::serve(webui_listener, webui_app).await.unwrap();
        });

        // Print startup info
        println!("\n========================================");
        println!("  AGN Studios v{}", env!("CARGO_PKG_VERSION"));
        println!("========================================");
        println!("  API:   http://127.0.0.1:{}", api_port);
        println!("  WebUI: http://127.0.0.1:{}", webui_port);
        println!("========================================\n");

        tokio::select! {
            _ = api_handle => info!("API server stopped"),
            _ = webui_handle => info!("WebUI server stopped"),
        }
    } else {
        println!("\n========================================");
        println!("  AGN Studios v{}", env!("CARGO_PKG_VERSION"));
        println!("========================================");
        println!("  API:   http://127.0.0.1:{}", api_port);
        println!("  WebUI: disabled");
        println!("========================================\n");

        api_handle.await.unwrap();
    }

    Ok(())
}