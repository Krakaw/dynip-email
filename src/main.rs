mod api;
mod smtp;
mod storage;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, Level};
use tracing_subscriber;

use storage::{models::Email, sqlite::SqliteBackend, StorageBackend};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    
    info!("🚀 Starting Temporary Mail Server");
    
    // Configuration
    let smtp_port = std::env::var("SMTP_PORT")
        .unwrap_or_else(|_| "2525".to_string())
        .parse::<u16>()?;
    
    let api_port = std::env::var("API_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()?;
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:emails.db".to_string());
    
    info!("📝 Configuration:");
    info!("  SMTP Port: {}", smtp_port);
    info!("  API Port: {}", api_port);
    info!("  Database: {}", database_url);
    
    // Initialize storage backend
    info!("💾 Initializing storage backend...");
    let storage: Arc<dyn StorageBackend> = Arc::new(SqliteBackend::new(&database_url).await?);
    info!("✅ Storage backend initialized");
    
    // Create broadcast channel for email notifications
    let (email_tx, _) = broadcast::channel::<Email>(100);
    
    // Start SMTP server
    info!("📧 Starting SMTP server...");
    let smtp_server = smtp::SmtpServer::new(storage.clone(), email_tx.clone());
    smtp_server.start(smtp_port).await?;
    info!("✅ SMTP server started on port {}", smtp_port);
    
    // Create API router
    info!("🌐 Creating API server...");
    let router = api::create_router(storage.clone(), email_tx);
    
    // Start API server
    info!("🚀 Starting API server on port {}...", api_port);
    info!("📱 Web interface available at: http://localhost:{}", api_port);
    info!("📬 SMTP server listening on port {}", smtp_port);
    
    api::start_server(router, api_port).await?;
    
    Ok(())
}
