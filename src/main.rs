mod api;
mod config;
mod smtp;
mod storage;

use anyhow::Result;
use config::Config;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::signal;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;

use storage::{models::Email, sqlite::SqliteBackend, StorageBackend};

#[tokio::main]
async fn main() -> Result<()> {
    
    // Initialize tracing with env filter
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .init();
    
    
    
    let config = Config::from_env()?;
    
    // Initialize storage backend
    let storage: Arc<dyn StorageBackend> = Arc::new(SqliteBackend::new(&config.database_url).await?);
    
    // Create broadcast channel for email notifications
    let (email_tx, _) = broadcast::channel::<Email>(100);
    
    // Start SMTP servers (non-TLS always, plus SSL ports if enabled)
    info!("📧 Starting SMTP servers...");
    let smtp_server = Arc::new(smtp::SmtpServer::new(
        storage.clone(),
        email_tx.clone(),
        config.domain_name.clone(),
        config.smtp_ssl.clone(),
        config.reject_non_domain_emails,
    ));
    
    // Start SMTP servers and wait for them to be ready
    match smtp_server.start_all(
        config.smtp_port,           // Non-TLS port (always listening)
        config.smtp_starttls_port,  // STARTTLS port (if SSL enabled)
        config.smtp_ssl_port,       // SMTPS port (if SSL enabled)
    ).await {
        Ok(_) => {
            if config.smtp_ssl.enabled {
                info!("✅ SMTP servers started on ports: {} (non-TLS), {} (STARTTLS), {} (SMTPS)", 
                      config.smtp_port, config.smtp_starttls_port, config.smtp_ssl_port);
            } else {
                info!("✅ SMTP server started on port {} (non-TLS only)", config.smtp_port);
            }
        }
        Err(e) => {
            error!("❌ Failed to start SMTP servers: {}", e);
            return Err(e);
        }
    }
    
    // Create API router
    let router = api::create_router(storage.clone(), email_tx, config.domain_name.clone());
    
    // Start API server
    info!("🚀 Starting API server on port {}...", config.api_port);
    
    // Set up graceful shutdown signal handling
    let smtp_server_clone = smtp_server.clone();
    let shutdown_signal = async move {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("🛑 Received Ctrl+C signal");
            },
            _ = terminate => {
                info!("🛑 Received terminate signal");
            },
        }
        
        // Shutdown SMTP servers
        info!("🛑 Shutting down SMTP servers...");
        smtp_server_clone.shutdown();
        
        // Give SMTP servers a moment to shutdown gracefully
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        info!("✅ SMTP servers shutdown complete");
    };

    // Start API server with graceful shutdown
    info!("✅ Server is running. Press Ctrl+C to stop gracefully...");
    
    // Run the server until shutdown signal is received
    match api::start_server_with_shutdown(router, config.api_port, shutdown_signal).await {
        Ok(_) => {
            info!("✅ Server shutdown completed gracefully");
        }
        Err(e) => {
            error!("❌ Server error: {}", e);
            return Err(e);
        }
    }
    
    // Force exit the process since SMTP servers don't support graceful shutdown
    // This ensures the application actually exits when Ctrl+C is pressed
    info!("🔄 Exiting application...");
    std::process::exit(0);
}
