pub mod handlers;
pub mod websocket;

use axum::{
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};
use tracing::info;

use crate::storage::{models::Email, StorageBackend};
use handlers::{get_email_by_id, get_emails_for_address};
use websocket::{websocket_handler, WsState};

/// Build the API router
pub fn create_router(
    storage: Arc<dyn StorageBackend>,
    email_sender: broadcast::Sender<Email>,
) -> Router {
    let ws_state = WsState {
        email_receiver: email_sender,
    };
    
    Router::new()
        // WebSocket route
        .route("/api/ws/:address", get(websocket_handler))
        .with_state(ws_state)
        // API routes with storage state
        .route("/api/emails/:address", get(get_emails_for_address))
        .route("/api/email/:id", get(get_email_by_id))
        .with_state(storage)
        // Serve static files
        .nest_service("/", ServeDir::new("src/frontend/static"))
        // CORS for development
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

/// Start the API server
pub async fn start_server(router: Router, port: u16) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    info!("Starting API server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;
    
    Ok(())
}

