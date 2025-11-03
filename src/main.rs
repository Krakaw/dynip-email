mod api;
mod config;
mod mcp;
mod smtp;
mod storage;
mod webhooks;

#[cfg(test)]
mod integration_tests;

use anyhow::Result;
use config::Config;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use mcp::EmailMcpServer;
use storage::{
    models::{Email, WebhookEvent},
    sqlite::SqliteBackend,
    StorageBackend,
};
use webhooks::WebhookTrigger;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic handler for better error reporting
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("💥 Application panicked: {}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!(
                "   at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }
    }));

    // Initialize tracing with env filter
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("🚀 Starting dynip-email server...");

    let config = match Config::from_env() {
        Ok(config) => {
            info!("✅ Configuration loaded successfully");
            config
        }
        Err(e) => {
            error!("❌ Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    // Initialize storage backend
    info!(
        "📊 Initializing database connection to: {}",
        config.database_url
    );
    let storage: Arc<dyn StorageBackend> = match SqliteBackend::new(&config.database_url).await {
        Ok(backend) => {
            info!("✅ Database connection established successfully");
            Arc::new(backend)
        }
        Err(e) => {
            error!("❌ Failed to initialize database: {}", e);
            return Err(e);
        }
    };

    // Create broadcast channels for email notifications and deletions
    let (email_tx, _) = broadcast::channel::<Email>(100);
    let (deletion_tx, _) = broadcast::channel::<(String, String)>(100);

    // Start email retention cleanup task if configured
    if let Some(retention_hours) = config.email_retention_hours {
        info!(
            "📅 Email retention enabled: emails older than {} hours will be deleted",
            retention_hours
        );
        let storage_clone = storage.clone();
        let deletion_tx_clone = deletion_tx.clone();
        let webhook_trigger = WebhookTrigger::new(storage.clone());
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Run every hour
            loop {
                interval.tick().await;
                match storage_clone
                    .delete_old_emails_with_details(retention_hours)
                    .await
                {
                    Ok(deleted_emails) => {
                        if !deleted_emails.is_empty() {
                            info!(
                                "🗑️  Email retention cleanup: deleted {} old email(s)",
                                deleted_emails.len()
                            );

                            // Send deletion notifications for each deleted email
                            for (email_id, address) in deleted_emails {
                                info!("📤 Broadcasting deletion notification for email {} to address {}", email_id, address);
                                let _ = deletion_tx_clone.send((email_id.clone(), address.clone()));

                                // Trigger webhooks for email deletion
                                // Extract mailbox name without domain for webhook lookup
                                let mailbox_name = address.split('@').next().unwrap_or(&address);
                                if let Err(e) = webhook_trigger
                                    .trigger_webhooks(mailbox_name, WebhookEvent::Deletion, None)
                                    .await
                                {
                                    error!("Failed to trigger deletion webhooks: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ Email retention cleanup failed: {}", e);
                    }
                }
            }
        });
    } else {
        info!("📅 Email retention disabled: emails will be kept indefinitely");
    }

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
    match smtp_server
        .start_all(
            config.smtp_port,          // Non-TLS port (always listening)
            config.smtp_starttls_port, // STARTTLS port (if SSL enabled)
            config.smtp_ssl_port,      // SMTPS port (if SSL enabled)
        )
        .await
    {
        Ok(_) => {
            if config.smtp_ssl.enabled {
                info!(
                    "✅ SMTP servers started on ports: {} (non-TLS), {} (STARTTLS), {} (SMTPS)",
                    config.smtp_port, config.smtp_starttls_port, config.smtp_ssl_port
                );
            } else {
                info!(
                    "✅ SMTP server started on port {} (non-TLS only)",
                    config.smtp_port
                );
            }
        }
        Err(e) => {
            error!("❌ Failed to start SMTP servers: {}", e);
            return Err(e);
        }
    }

    // Create webhook trigger
    let webhook_trigger = webhooks::WebhookTrigger::new(storage.clone());

    // Create API router
    let router = api::create_router(
        storage.clone(),
        email_tx,
        deletion_tx,
        config.domain_name.clone(),
        webhook_trigger,
    );

    // Start MCP server if enabled
    if config.mcp_enabled {
        info!("🔌 Starting MCP server on port {}...", config.mcp_port);
        let mcp_server = EmailMcpServer::new(storage.clone());
        let mcp_port = config.mcp_port;
        tokio::spawn(async move {
            if let Err(e) = mcp_server.start(mcp_port).await {
                error!("❌ MCP server error: {}", e);
            }
        });
    } else {
        info!("🔌 MCP server disabled");
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::storage::{models::Email, sqlite::SqliteBackend};
    use std::env;

    /// Load configuration from environment variables without loading .env file
    /// This is used for tests to avoid interference from .env files
    fn from_env_test() -> Result<Config> {
        // Non-TLS SMTP port (always listening)
        let smtp_port = std::env::var("SMTP_PORT")
            .unwrap_or_else(|_| "2525".to_string())
            .parse()?;

        // STARTTLS port
        let smtp_starttls_port = std::env::var("SMTP_STARTTLS_PORT")
            .unwrap_or_else(|_| "587".to_string())
            .parse()?;

        // SSL/TLS port
        let smtp_ssl_port = std::env::var("SMTP_SSL_PORT")
            .unwrap_or_else(|_| "465".to_string())
            .parse()?;

        // API port
        let api_port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;

        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:emails.db".to_string());

        let domain_name =
            std::env::var("DOMAIN_NAME").unwrap_or_else(|_| "tempmail.local".to_string());

        let email_retention_hours = std::env::var("EMAIL_RETENTION_HOURS")
            .ok()
            .and_then(|s| s.parse().ok());

        let reject_non_domain_emails = std::env::var("REJECT_NON_DOMAIN_EMAILS")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let smtp_ssl = crate::config::SmtpSslConfig {
            enabled: std::env::var("SMTP_SSL_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            cert_path: std::env::var("SMTP_SSL_CERT_PATH")
                .ok()
                .map(std::path::PathBuf::from),
            key_path: std::env::var("SMTP_SSL_KEY_PATH")
                .ok()
                .map(std::path::PathBuf::from),
        };

        Ok(Config {
            smtp_port,
            smtp_starttls_port,
            smtp_ssl_port,
            api_port,
            database_url,
            domain_name,
            email_retention_hours,
            reject_non_domain_emails,
            smtp_ssl,
            mcp_enabled: false,
            mcp_port: 3001,
        })
    }

    #[test]
    fn test_config_loading() {
        // Test that config can be loaded from environment
        env::set_var("SMTP_PORT", "2525");
        env::set_var("API_PORT", "3000");
        env::set_var("DATABASE_URL", "sqlite:test.db");
        env::set_var("DOMAIN_NAME", "test.local");

        let config = from_env_test().unwrap();
        assert_eq!(config.smtp_port, 2525);
        assert_eq!(config.api_port, 3000);
        assert_eq!(config.database_url, "sqlite:test.db");
        assert_eq!(config.domain_name, "test.local");
    }

    #[test]
    fn test_config_with_ssl_enabled() {
        env::set_var("SMTP_SSL_ENABLED", "true");
        env::set_var("SMTP_SSL_CERT_PATH", "/path/to/cert.pem");
        env::set_var("SMTP_SSL_KEY_PATH", "/path/to/key.pem");

        let config = from_env_test().unwrap();
        assert!(config.smtp_ssl.enabled);
        assert_eq!(
            config.smtp_ssl.cert_path,
            Some(std::path::PathBuf::from("/path/to/cert.pem"))
        );
        assert_eq!(
            config.smtp_ssl.key_path,
            Some(std::path::PathBuf::from("/path/to/key.pem"))
        );
    }

    #[test]
    fn test_config_with_retention_hours() {
        env::set_var("EMAIL_RETENTION_HOURS", "24");

        let config = from_env_test().unwrap();
        assert_eq!(config.email_retention_hours, Some(24));
    }

    #[test]
    fn test_config_with_reject_non_domain_emails() {
        env::set_var("REJECT_NON_DOMAIN_EMAILS", "true");

        let config = from_env_test().unwrap();
        assert!(config.reject_non_domain_emails);
    }

    #[tokio::test]
    async fn test_storage_backend_creation() {
        // Use in-memory database for tests
        let database_url = "sqlite::memory:";

        let storage: Arc<dyn StorageBackend> =
            Arc::new(SqliteBackend::new(&database_url).await.unwrap());

        // Test that we can store and retrieve an email
        let email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );

        storage.store_email(email.clone()).await.unwrap();

        let retrieved_emails = storage
            .get_emails_for_address("test@test.local")
            .await
            .unwrap();
        assert_eq!(retrieved_emails.len(), 1);
        assert_eq!(retrieved_emails[0].id, email.id);
    }

    #[tokio::test]
    async fn test_email_retention_cleanup() {
        // Use in-memory database for tests
        let database_url = "sqlite::memory:";

        let storage: Arc<dyn StorageBackend> =
            Arc::new(SqliteBackend::new(&database_url).await.unwrap());

        // Create an old email
        let mut old_email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Old Subject".to_string(),
            "Old body".to_string(),
            None,
            vec![],
        );
        old_email.timestamp = chrono::Utc::now() - chrono::Duration::hours(25);

        // Create a new email
        let new_email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "New Subject".to_string(),
            "New body".to_string(),
            None,
            vec![],
        );

        // Store both emails
        storage.store_email(old_email.clone()).await.unwrap();
        storage.store_email(new_email.clone()).await.unwrap();

        // Verify both emails exist
        let emails = storage
            .get_emails_for_address("test@test.local")
            .await
            .unwrap();
        assert_eq!(emails.len(), 2);

        // Delete emails older than 24 hours
        let deleted_details = storage.delete_old_emails_with_details(24).await.unwrap();
        assert_eq!(deleted_details.len(), 1);
        assert_eq!(deleted_details[0].0, old_email.id);
        assert_eq!(deleted_details[0].1, old_email.to);

        // Verify only the new email remains
        let emails = storage
            .get_emails_for_address("test@test.local")
            .await
            .unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].id, new_email.id);
    }

    #[tokio::test]
    async fn test_broadcast_channel_creation() {
        let (email_tx, mut email_rx) = broadcast::channel::<Email>(100);
        let (deletion_tx, mut deletion_rx) = broadcast::channel::<(String, String)>(100);

        // Test that channels are created successfully
        assert_eq!(email_tx.receiver_count(), 1);
        assert_eq!(deletion_tx.receiver_count(), 1);

        // Test that we can send and receive messages
        let email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );

        email_tx.send(email.clone()).unwrap();
        let received_email = email_rx.recv().await.unwrap();
        assert_eq!(received_email.id, email.id);

        deletion_tx
            .send(("test-id".to_string(), "test@test.local".to_string()))
            .unwrap();
        let (id, address) = deletion_rx.recv().await.unwrap();
        assert_eq!(id, "test-id");
        assert_eq!(address, "test@test.local");
    }

    #[test]
    fn test_email_model_creation() {
        let email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            Some("Raw email content".to_string()),
            vec![],
        );

        assert_eq!(email.to, "test@test.local");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "Test Subject");
        assert_eq!(email.body, "Test body");
        assert_eq!(email.raw, Some("Raw email content".to_string()));
        assert!(email.attachments.is_empty());
        assert!(!email.id.is_empty());
    }

    #[test]
    fn test_email_with_attachments() {
        let attachments = vec![crate::storage::models::Attachment {
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 100,
            content: "dGVzdCBjb250ZW50".to_string(),
        }];

        let email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            attachments.clone(),
        );

        assert_eq!(email.attachments.len(), 1);
        assert_eq!(email.attachments[0].filename, "test.txt");
        assert_eq!(email.attachments[0].content_type, "text/plain");
        assert_eq!(email.attachments[0].size, 100);
        assert_eq!(email.attachments[0].content, "dGVzdCBjb250ZW50");
    }

    #[test]
    fn test_config_default_values() {
        // Clear environment variables to test defaults
        env::remove_var("SMTP_PORT");
        env::remove_var("SMTP_STARTTLS_PORT");
        env::remove_var("SMTP_SSL_PORT");
        env::remove_var("API_PORT");
        env::remove_var("DATABASE_URL");
        env::remove_var("DOMAIN_NAME");
        env::remove_var("EMAIL_RETENTION_HOURS");
        env::remove_var("REJECT_NON_DOMAIN_EMAILS");
        env::remove_var("SMTP_SSL_ENABLED");
        env::remove_var("SMTP_SSL_CERT_PATH");
        env::remove_var("SMTP_SSL_KEY_PATH");

        let config = from_env_test().unwrap();

        assert_eq!(config.smtp_port, 2525);
        assert_eq!(config.api_port, 3000);
        assert_eq!(config.database_url, "sqlite:emails.db");
        assert_eq!(config.domain_name, "tempmail.local");
        assert_eq!(config.email_retention_hours, None);
        assert_eq!(config.reject_non_domain_emails, false);
        assert_eq!(config.smtp_ssl.enabled, false);
    }
}
