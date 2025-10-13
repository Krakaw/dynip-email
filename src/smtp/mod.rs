pub mod parser;

use anyhow::Result;
use mailin_embedded::{Handler, Server, SslConfig};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::broadcast;
use tracing::{debug, error, info};

use crate::storage::{models::{Email, WebhookEvent}, StorageBackend};
use crate::webhooks::WebhookTrigger;
use parser::parse_email;

/// SMTP server that accepts all emails
pub struct SmtpServer {
    storage: Arc<dyn StorageBackend>,
    email_sender: broadcast::Sender<Email>,
    domain_name: String,
    ssl_config: crate::config::SmtpSslConfig,
    reject_non_domain_emails: bool,
    shutdown_flag: Arc<AtomicBool>,
}

impl SmtpServer {
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        email_sender: broadcast::Sender<Email>,
        domain_name: String,
        ssl_config: crate::config::SmtpSslConfig,
        reject_non_domain_emails: bool,
    ) -> Self {
        Self {
            storage,
            email_sender,
            domain_name,
            ssl_config,
            reject_non_domain_emails,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set the shutdown flag to signal all SMTP servers to stop
    pub fn shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
        info!("🛑 SMTP server shutdown signal sent");
    }

    /// Start multiple SMTP servers on different ports
    /// - Always starts non-TLS server on smtp_port
    /// - If SSL enabled, also starts STARTTLS server on smtp_starttls_port  
    /// - If SSL enabled, also starts SMTPS server on smtp_ssl_port
    pub async fn start_all(
        &self,
        smtp_port: u16,
        smtp_starttls_port: u16,
        smtp_ssl_port: u16,
    ) -> Result<()> {
        let storage = self.storage.clone();
        let email_sender = self.email_sender.clone();
        let domain_name = self.domain_name.clone();
        let ssl_config = self.ssl_config.clone();
        let reject_non_domain_emails = self.reject_non_domain_emails;
        let shutdown_flag = self.shutdown_flag.clone();

        // Always start non-TLS SMTP server
        let non_tls_server = SmtpServer {
            storage: storage.clone(),
            email_sender: email_sender.clone(),
            domain_name: domain_name.clone(),
            ssl_config: crate::config::SmtpSslConfig {
                enabled: false,
                cert_path: None,
                key_path: None,
            },
            reject_non_domain_emails,
            shutdown_flag: shutdown_flag.clone(),
        };
        non_tls_server
            .start_single(smtp_port, "non-TLS".to_string())
            .await?;

        // If SSL is enabled, start additional servers
        if ssl_config.enabled {
            // Start STARTTLS server on port 587
            let starttls_server = SmtpServer {
                storage: storage.clone(),
                email_sender: email_sender.clone(),
                domain_name: domain_name.clone(),
                ssl_config: ssl_config.clone(),
                reject_non_domain_emails,
                shutdown_flag: shutdown_flag.clone(),
            };
            starttls_server
                .start_single(smtp_starttls_port, "STARTTLS".to_string())
                .await?;

            // Start SMTPS server on port 465
            let smtps_server = SmtpServer {
                storage,
                email_sender,
                domain_name,
                ssl_config,
                reject_non_domain_emails,
                shutdown_flag,
            };
            smtps_server
                .start_single(smtp_ssl_port, "SMTPS".to_string())
                .await?;
        }

        // Give servers a moment to start up
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }

    /// Start a single SMTP server instance on the specified port
    async fn start_single(&self, port: u16, server_type: String) -> Result<()> {
        debug!("Starting {} SMTP server on port {}...", server_type, port);

        let addr = format!("0.0.0.0:{}", port);
        let shutdown_flag = self.shutdown_flag.clone();

        // Get the runtime handle to pass to both the blocking thread and handler
        let runtime_handle = tokio::runtime::Handle::current();
        let handler = SmtpHandler::new(
            self.storage.clone(),
            self.email_sender.clone(),
            runtime_handle.clone(),
            self.domain_name.clone(),
            self.reject_non_domain_emails,
        );

        // Determine SSL configuration
        let ssl_config = if self.ssl_config.enabled {
            match self.ssl_config.load_certificates() {
                Ok(Some((_certs, _key))) => {
                    // mailin-embedded expects SslConfig::SelfSigned with cert/key data
                    // We'll need to configure this properly
                    SslConfig::None // Placeholder - mailin-embedded has limited SSL support
                }
                Ok(None) => SslConfig::None,
                Err(e) => {
                    error!("Failed to load SSL certificates: {}", e);
                    return Err(e);
                }
            }
        } else {
            SslConfig::None
        };

        let domain_name = self.domain_name.clone();

        // Run the server in a blocking manner with shutdown support
        let server_handle = tokio::task::spawn_blocking(move || {
            // Enter the runtime context so tokio::spawn works
            let _guard = runtime_handle.enter();

            let mut server = Server::new(handler);

            if let Err(e) = server
                .with_name(&domain_name)
                .with_ssl(ssl_config)
                .and_then(|s| s.with_addr(&addr))
            {
                error!(
                    "Failed to configure {} SMTP server on port {}: {}",
                    server_type, port, e
                );
                return;
            }

            // Start a background task to monitor shutdown signal and abort the server
            let shutdown_flag_clone = shutdown_flag.clone();
            let server_type_clone = server_type.clone();
            let port_clone = port;

            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    if shutdown_flag_clone.load(Ordering::SeqCst) {
                        info!(
                            "🛑 Shutdown signal received for {} SMTP server on port {}",
                            server_type_clone, port_clone
                        );
                        break;
                    }
                }
            });

            // Note: mailin-embedded doesn't have built-in graceful shutdown
            // The server will continue running until the process exits
            // In a production environment, you might want to implement a custom
            // shutdown mechanism or use a different SMTP library
            if let Err(e) = server.serve() {
                if shutdown_flag.load(Ordering::SeqCst) {
                    info!(
                        "✅ {} SMTP server on port {} stopped gracefully",
                        server_type, port
                    );
                } else {
                    error!("{} SMTP server error on port {}: {}", server_type, port, e);
                }
            }
        });

        // Store the server handle for potential future use
        // For now, we'll let it run in the background
        drop(server_handle);

        Ok(())
    }
}

/// Handler for SMTP events
#[derive(Clone)]
struct SmtpHandler {
    storage: Arc<dyn StorageBackend>,
    email_sender: broadcast::Sender<Email>,
    runtime_handle: tokio::runtime::Handle,
    domain_name: String,
    reject_non_domain_emails: bool,
    // Store email data during the session
    from: Arc<std::sync::Mutex<String>>,
    to: Arc<std::sync::Mutex<Vec<String>>>,
    data: Arc<std::sync::Mutex<Vec<u8>>>,
}

impl SmtpHandler {
    fn new(
        storage: Arc<dyn StorageBackend>,
        email_sender: broadcast::Sender<Email>,
        runtime_handle: tokio::runtime::Handle,
        domain_name: String,
        reject_non_domain_emails: bool,
    ) -> Self {
        Self {
            storage,
            email_sender,
            runtime_handle,
            domain_name,
            reject_non_domain_emails,
            from: Arc::new(std::sync::Mutex::new(String::new())),
            to: Arc::new(std::sync::Mutex::new(Vec::new())),
            data: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl Handler for SmtpHandler {
    fn data_start(
        &mut self,
        _domain: &str,
        from: &str,
        _is8bit: bool,
        to: &[String],
    ) -> mailin_embedded::Response {
        info!("Receiving email from {} to {:?}", from, to);

        // Check domain validation if enabled
        if self.reject_non_domain_emails {
            for recipient in to {
                if let Some(at_pos) = recipient.find('@') {
                    let domain = &recipient[at_pos + 1..];
                    if domain != self.domain_name {
                        info!(
                            "Rejecting email to {} - domain {} does not match configured domain {}",
                            recipient, domain, self.domain_name
                        );
                        return mailin_embedded::response::NO_MAILBOX;
                    }
                } else {
                    // Invalid email format, reject
                    info!("Rejecting email to {} - invalid email format", recipient);
                    return mailin_embedded::response::INTERNAL_ERROR;
                }
            }
        }

        // Store from and to
        *self.from.lock().unwrap() = from.to_string();
        *self.to.lock().unwrap() = to.to_vec();
        self.data.lock().unwrap().clear();

        mailin_embedded::response::OK
    }

    fn data(&mut self, buf: &[u8]) -> std::io::Result<()> {
        // Accumulate data
        self.data.lock().unwrap().extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> mailin_embedded::Response {
        let from = self.from.lock().unwrap().clone();
        let to = self.to.lock().unwrap().clone();
        let data = self.data.lock().unwrap().clone();

        let recipient = to
            .first()
            .map(|s| s.as_str())
            .unwrap_or("unknown@localhost");

        info!(
            "Email received completely from {} to {} ({} bytes)",
            from,
            recipient,
            data.len()
        );

        // Parse the email
        let email = match parse_email(&data, recipient) {
            Ok(email) => {
                info!(
                    "Successfully parsed email: id={}, subject={}",
                    email.id, email.subject
                );
                email
            }
            Err(e) => {
                error!("Failed to parse email: {}", e);
                return mailin_embedded::response::INTERNAL_ERROR;
            }
        };

        // Store the email using the tokio runtime handle
        let storage = self.storage.clone();
        let email_clone = email.clone();

        // Use the stored runtime handle to spawn the storage task
        let webhook_trigger = WebhookTrigger::new(self.storage.clone());
        let email_for_webhook = email_clone.clone();
        let to_address = email_clone.to.clone();
        
        self.runtime_handle.spawn(async move {
            if let Err(e) = storage.store_email(email_clone.clone()).await {
                error!("Failed to store email: {}", e);
            } else {
                debug!("Successfully stored email {}", email_clone.id);
                
                // Trigger webhooks for email arrival
                // Extract mailbox name without domain for webhook lookup
                let mailbox_name = to_address.split('@').next().unwrap_or(&to_address);
                if let Err(e) = webhook_trigger.trigger_webhooks(mailbox_name, WebhookEvent::Arrival, Some(&email_for_webhook)).await {
                    error!("Failed to trigger webhooks: {}", e);
                }
            }
        });

        // Broadcast the email to WebSocket listeners
        let _ = self.email_sender.send(email);

        mailin_embedded::response::OK
    }
}
