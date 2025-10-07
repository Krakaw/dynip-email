pub mod parser;

use anyhow::Result;
use mailin_embedded::{Handler, Server, SslConfig};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::storage::{models::Email, StorageBackend};
use parser::parse_email;

/// SMTP server that accepts all emails
pub struct SmtpServer {
    storage: Arc<dyn StorageBackend>,
    email_sender: broadcast::Sender<Email>,
}

impl SmtpServer {
    pub fn new(storage: Arc<dyn StorageBackend>, email_sender: broadcast::Sender<Email>) -> Self {
        Self {
            storage,
            email_sender,
        }
    }
    
    /// Start the SMTP server on the specified port
    pub async fn start(self, port: u16) -> Result<()> {
        info!("Starting SMTP server on port {}", port);
        
        let addr = format!("0.0.0.0:{}", port);
        
        // Get the runtime handle to pass to both the blocking thread and handler
        let runtime_handle = tokio::runtime::Handle::current();
        let handler = SmtpHandler::new(self.storage, self.email_sender, runtime_handle.clone());
        
        // Run the server in a blocking manner
        tokio::task::spawn_blocking(move || {
            // Enter the runtime context so tokio::spawn works
            let _guard = runtime_handle.enter();
            
            let mut server = Server::new(handler);
            
            if let Err(e) = server
                .with_name("tempmail.local")
                .with_ssl(SslConfig::None)
                .and_then(|s| s.with_addr(&addr))
            {
                error!("Failed to configure SMTP server: {}", e);
                return;
            }
            
            info!("SMTP server configured, starting to serve...");
            server.serve().unwrap();
        });
        
        info!("SMTP server started successfully on port {}", port);
        Ok(())
    }
}

/// Handler for SMTP events
#[derive(Clone)]
struct SmtpHandler {
    storage: Arc<dyn StorageBackend>,
    email_sender: broadcast::Sender<Email>,
    runtime_handle: tokio::runtime::Handle,
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
    ) -> Self {
        Self {
            storage,
            email_sender,
            runtime_handle,
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
        
        let recipient = to.first().map(|s| s.as_str()).unwrap_or("unknown@localhost");
        
        info!("Email received completely from {} to {} ({} bytes)", from, recipient, data.len());
        
        // Parse the email
        let email = match parse_email(&data, recipient) {
            Ok(email) => {
                info!("Successfully parsed email: id={}, subject={}", email.id, email.subject);
                email
            },
            Err(e) => {
                error!("Failed to parse email: {}", e);
                return mailin_embedded::response::INTERNAL_ERROR;
            }
        };
        
        // Store the email using the tokio runtime handle
        let storage = self.storage.clone();
        let email_clone = email.clone();
        
        // Use the stored runtime handle to spawn the storage task
        info!("Spawning storage task on tokio runtime");
        self.runtime_handle.spawn(async move {
            info!("Storing email {} in database", email_clone.id);
            if let Err(e) = storage.store_email(email_clone.clone()).await {
                error!("Failed to store email: {}", e);
            } else {
                info!("Successfully stored email {}", email_clone.id);
            }
        });
        
        // Broadcast the email to WebSocket listeners
        info!("Broadcasting email to WebSocket listeners");
        let _ = self.email_sender.send(email);
        
        mailin_embedded::response::OK
    }
}

