pub mod handlers;
pub mod websocket;

use axum::{routing::{get, post, put, delete}, Router};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};
use tracing::info;

use crate::storage::{models::Email, StorageBackend};
use crate::webhooks::WebhookTrigger;
use handlers::{
    get_email_by_id, get_emails_for_address, delete_email, AppConfig,
    create_webhook, get_webhooks_for_mailbox, get_webhook_by_id,
    update_webhook, delete_webhook, test_webhook
};
use websocket::{websocket_handler, WsState};

/// Build the API router
pub fn create_router(
    storage: Arc<dyn StorageBackend>,
    email_sender: broadcast::Sender<Email>,
    deletion_sender: broadcast::Sender<(String, String)>,
    domain_name: String,
    webhook_trigger: WebhookTrigger,
) -> Router {
    let ws_state = WsState {
        email_receiver: email_sender.clone(),
        deletion_sender,
        domain_name: domain_name.clone(),
    };

    let app_config = AppConfig { domain_name };

    // Create combined state for routes that need both storage and config
    let combined_state = (storage.clone(), app_config.clone());
    
    // Create state for delete email route (storage + webhook_trigger)
    let delete_email_state = (storage.clone(), webhook_trigger);

    Router::new()
        // WebSocket route (needs domain for normalization)
        .route("/api/ws/:address", get(websocket_handler))
        .with_state(ws_state)
        // API routes with combined state (storage + config)
        .route("/api/emails/:address", get(get_emails_for_address))
        .with_state(combined_state)
        // Email by ID doesn't need domain normalization
        .route("/api/email/:id", get(get_email_by_id))
        .with_state(storage.clone())
        // Delete email route needs storage + webhook_trigger
        .route("/api/email/:id", delete(delete_email))
        .with_state(delete_email_state)
        // Webhook routes
        .route("/api/webhooks", post(create_webhook))
        .with_state(storage.clone())
        .route("/api/webhooks/:address", get(get_webhooks_for_mailbox))
        .with_state(storage.clone())
        .route("/api/webhook/:id", get(get_webhook_by_id))
        .with_state(storage.clone())
        .route("/api/webhook/:id", put(update_webhook))
        .with_state(storage.clone())
        .route("/api/webhook/:id", delete(delete_webhook))
        .with_state(storage.clone())
        .route("/api/webhook/:id/test", post(test_webhook))
        .with_state(storage)
        // Serve static files
        .nest_service("/", ServeDir::new("static"))
        // CORS for development
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

/// Start the API server
#[allow(dead_code)]
pub async fn start_server(router: Router, port: u16) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    info!("Starting API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

/// Start the API server with graceful shutdown support
pub async fn start_server_with_shutdown(
    router: Router,
    port: u16,
    shutdown_signal: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    info!("Starting API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Create a shutdown signal that can be used to gracefully stop the server
    let shutdown_signal = async {
        shutdown_signal.await;
        info!("🛑 Shutdown signal received, stopping server gracefully...");
    };

    // Start the server with graceful shutdown
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    info!("✅ API server stopped gracefully");
    Ok(())
}
