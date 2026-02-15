pub mod admin;
pub mod handlers;
pub mod websocket;

use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};
use tracing::info;

use crate::auth::{self, AuthConfig};
use crate::rate_limit;
use crate::storage::{models::Email, StorageBackend};
use crate::webhooks::WebhookTrigger;
use admin::{delete_rate_limit, get_rate_limit, get_rate_limit_stats, set_rate_limit};
use handlers::{
    check_mailbox_status, claim_mailbox, create_webhook, delete_email, delete_webhook,
    get_email_by_id, get_emails_for_address, get_webhook_by_id, get_webhooks_for_mailbox,
    release_mailbox, search_emails, test_webhook, update_webhook, AppConfig,
};
use websocket::{websocket_handler, WsState};

/// Build the API router
pub fn create_router(
    storage: Arc<dyn StorageBackend>,
    email_sender: broadcast::Sender<Email>,
    deletion_sender: broadcast::Sender<(String, String)>,
    domain_name: String,
    webhook_trigger: WebhookTrigger,
    auth_config: AuthConfig,
) -> Router {
    let ws_state = WsState {
        email_receiver: email_sender.clone(),
        deletion_sender,
        domain_name: domain_name.clone(),
    };

    let app_config = AppConfig { domain_name };

    // Create state for delete email route (storage + webhook_trigger)
    let delete_email_state = (storage.clone(), webhook_trigger);

    // Create auth state
    let auth_state = (storage.clone(), auth_config.clone());

    // Build protected routes (require auth when enabled)
    let protected_routes = Router::new()
        // Mailbox routes
        .route("/api/mailbox/:address/status", get(check_mailbox_status))
        .with_state((storage.clone(), app_config.clone()))
        .route("/api/mailbox/:address/claim", post(claim_mailbox))
        .with_state((storage.clone(), app_config.clone()))
        .route("/api/mailbox/:address/release", post(release_mailbox))
        .with_state((storage.clone(), app_config.clone()))
        // API routes with combined state (storage + config)
        .route("/api/emails/:address", get(get_emails_for_address))
        .with_state((storage.clone(), app_config.clone()))
        // Search emails (needs storage + config for mailbox normalization)
        .route("/api/search", get(search_emails))
        .with_state((storage.clone(), app_config.clone()))
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
        .with_state(storage.clone())
        // Admin routes for rate limiting
        .route("/api/admin/rate-limit/:address", get(get_rate_limit))
        .with_state(storage.clone())
        .route("/api/admin/rate-limit/:address", post(set_rate_limit))
        .with_state(storage.clone())
        .route("/api/admin/rate-limit/:address", delete(delete_rate_limit))
        .with_state(storage.clone())
        .route(
            "/api/admin/rate-limit/:address/stats",
            get(get_rate_limit_stats),
        )
        .with_state(storage.clone())
        // Apply rate limiting middleware first
        .layer(middleware::from_fn_with_state(
            storage.clone(),
            rate_limit::rate_limit_middleware,
        ))
        // Apply auth middleware to protected routes
        .layer(middleware::from_fn_with_state(
            auth_config.clone(),
            auth::require_auth,
        ));

    // Build auth routes (public, no auth required)
    let auth_routes = Router::new()
        .route("/api/auth/status", get(auth::status))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        .with_state(auth_state)
        // Apply auth config middleware so AuthenticatedUser extractor can access config
        .layer(middleware::from_fn_with_state(
            auth_config.clone(),
            auth::auth_config_middleware,
        ));

    Router::new()
        // WebSocket route (needs domain for normalization)
        .route("/api/ws/:address", get(websocket_handler))
        .with_state(ws_state)
        // Merge auth routes (public)
        .merge(auth_routes)
        // Merge protected routes
        .merge(protected_routes)
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
