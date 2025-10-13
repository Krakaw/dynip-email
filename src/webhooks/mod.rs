use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::storage::{models::{Email, Webhook, WebhookEvent}, StorageBackend};
use std::sync::Arc;

/// Webhook trigger system for sending HTTP POST requests
#[derive(Clone)]
pub struct WebhookTrigger {
    client: Client,
    storage: Arc<dyn StorageBackend>,
}

impl WebhookTrigger {
    /// Create a new webhook trigger
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, storage }
    }

    /// Trigger webhooks for a specific event and mailbox
    pub async fn trigger_webhooks(&self, address: &str, event: WebhookEvent, email: Option<&Email>) -> Result<()> {
        let webhooks = self.storage.get_active_webhooks_for_event(address, event.clone()).await?;
        
        if webhooks.is_empty() {
            return Ok(());
        }

        info!(
            "Triggering {} webhook(s) for event {:?} on mailbox {}",
            webhooks.len(),
            event,
            address
        );

        // Trigger webhooks concurrently
        let mut handles = Vec::new();
        
        for webhook in webhooks {
            let client = self.client.clone();
            let payload = self.create_webhook_payload(&event, email, &webhook);
            let webhook_url = self.normalize_webhook_url(&webhook.webhook_url)?;
            let webhook_id = webhook.id.clone();
            
            let handle = tokio::spawn(async move {
                Self::send_webhook_with_retry(client, &webhook_url, payload, &webhook_id).await
            });
            
            handles.push(handle);
        }

        // Wait for all webhooks to complete (don't fail if some fail)
        for handle in handles {
            if let Err(e) = handle.await {
                error!("Webhook task failed: {}", e);
            }
        }

        Ok(())
    }

    /// Create webhook payload based on event type
    fn create_webhook_payload(&self, event: &WebhookEvent, email: Option<&Email>, webhook: &Webhook) -> Value {
        let mut payload = json!({
            "event": event.as_str(),
            "mailbox": webhook.mailbox_address,
            "webhook_id": webhook.id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        if let Some(email) = email {
            payload["email"] = json!({
                "id": email.id,
                "to": email.to,
                "from": email.from,
                "subject": email.subject,
                "body": email.body,
                "timestamp": email.timestamp.to_rfc3339(),
                "attachments": email.attachments.len()
            });
        }

        payload
    }

    /// Normalize webhook URL by adding http:// if no scheme is provided
    fn normalize_webhook_url(&self, url: &str) -> Result<String> {
        if url.starts_with("http://") || url.starts_with("https://") {
            Ok(url.to_string())
        } else {
            // Assume http:// for URLs without scheme
            Ok(format!("http://{}", url))
        }
    }

    /// Send webhook with retry logic
    async fn send_webhook_with_retry(
        client: Client,
        url: &str,
        payload: Value,
        webhook_id: &str,
    ) -> Result<()> {
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 1..=max_retries {
            match client
                .post(url)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Webhook {} sent successfully to {}", webhook_id, url);
                        return Ok(());
                    } else {
                        warn!(
                            "Webhook {} failed with status {}",
                            webhook_id,
                            response.status()
                        );
                        last_error = Some(format!("HTTP {}", response.status()));
                    }
                }
                Err(e) => {
                    warn!("Webhook {} attempt {} failed: {}", webhook_id, attempt, e);
                    last_error = Some(e.to_string());
                }
            }

            if attempt < max_retries {
                let delay = Duration::from_secs(2_u64.pow(attempt - 1));
                info!("Retrying webhook {} in {:?}", webhook_id, delay);
                sleep(delay).await;
            }
        }

        error!(
            "Webhook {} failed after {} attempts. Last error: {}",
            webhook_id,
            max_retries,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        );

        Ok(()) // Don't propagate webhook failures
    }

    /// Test a webhook by sending a test payload
    pub async fn test_webhook(&self, webhook: &Webhook) -> Result<bool> {
        let test_payload = json!({
            "event": "test",
            "mailbox": webhook.mailbox_address,
            "webhook_id": webhook.id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "message": "This is a test webhook payload"
        });

        // Normalize URL - add http:// if no scheme is provided
        let url = self.normalize_webhook_url(&webhook.webhook_url)?;

        match self.client
            .post(&url)
            .json(&test_payload)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Test webhook {} successful", webhook.id);
                    Ok(true)
                } else {
                    warn!(
                        "Test webhook {} failed with status {}",
                        webhook.id,
                        response.status()
                    );
                    Ok(false)
                }
            }
            Err(e) => {
                error!("Test webhook {} failed: {}", webhook.id, e);
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{Email, Webhook, WebhookEvent};

    #[tokio::test]
    async fn test_webhook_payload_creation() {
        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );

        let webhook = Webhook::new(
            "test@example.com".to_string(),
            "https://example.com/webhook".to_string(),
            vec![WebhookEvent::Arrival],
        );

        // Create a mock storage backend for testing
        let storage = Arc::new(crate::storage::sqlite::SqliteBackend::new("sqlite::memory:").await.unwrap());
        let trigger = WebhookTrigger {
            client: Client::new(),
            storage,
        };

        let payload = trigger.create_webhook_payload(&WebhookEvent::Arrival, Some(&email), &webhook);

        assert_eq!(payload["event"], "arrival");
        assert_eq!(payload["mailbox"], "test@example.com");
        assert_eq!(payload["webhook_id"], webhook.id);
        assert!(payload["email"].is_object());
        assert_eq!(payload["email"]["id"], email.id);
    }

    #[test]
    fn test_webhook_event_serialization() {
        assert_eq!(WebhookEvent::Arrival.as_str(), "arrival");
        assert_eq!(WebhookEvent::Deletion.as_str(), "deletion");
        assert_eq!(WebhookEvent::Read.as_str(), "read");

        assert_eq!(WebhookEvent::from_str("arrival"), Some(WebhookEvent::Arrival));
        assert_eq!(WebhookEvent::from_str("deletion"), Some(WebhookEvent::Deletion));
        assert_eq!(WebhookEvent::from_str("read"), Some(WebhookEvent::Read));
        assert_eq!(WebhookEvent::from_str("invalid"), None);
    }

    #[tokio::test]
    async fn test_webhook_http_delivery_success() {
        use mockito::{Server, Mock};
        
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/webhook")
            .with_status(200)
            .with_header("content-type", "application/json")
            .create_async()
            .await;

        let webhook_url = format!("{}/webhook", server.url());
        let webhook = Webhook::new(
            "test".to_string(),
            webhook_url,
            vec![WebhookEvent::Arrival],
        );

        let storage = Arc::new(crate::storage::sqlite::SqliteBackend::new("sqlite::memory:").await.unwrap());
        let trigger = WebhookTrigger::new(storage);

        let result = trigger.test_webhook(&webhook).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        _mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_webhook_http_delivery_failure() {
        use mockito::{Server, Mock};
        
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/webhook")
            .with_status(500)
            .create_async()
            .await;

        let webhook_url = format!("{}/webhook", server.url());
        let webhook = Webhook::new(
            "test".to_string(),
            webhook_url,
            vec![WebhookEvent::Arrival],
        );

        let storage = Arc::new(crate::storage::sqlite::SqliteBackend::new("sqlite::memory:").await.unwrap());
        let trigger = WebhookTrigger::new(storage);

        let result = trigger.test_webhook(&webhook).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());

        _mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_webhook_http_delivery_timeout() {
        use mockito::{Server, Mock};
        
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/webhook")
            .with_status(200)
            .create_async()
            .await;

        let webhook_url = format!("{}/webhook", server.url());
        let webhook = Webhook::new(
            "test".to_string(),
            webhook_url,
            vec![WebhookEvent::Arrival],
        );

        let storage = Arc::new(crate::storage::sqlite::SqliteBackend::new("sqlite::memory:").await.unwrap());
        let trigger = WebhookTrigger::new(storage);

        let result = trigger.test_webhook(&webhook).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        _mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_webhook_payload_without_email() {
        let webhook = Webhook::new(
            "test".to_string(),
            "http://localhost:3009".to_string(),
            vec![WebhookEvent::Deletion],
        );

        let storage = Arc::new(crate::storage::sqlite::SqliteBackend::new("sqlite::memory:").await.unwrap());
        let trigger = WebhookTrigger::new(storage);
        let payload = trigger.create_webhook_payload(&WebhookEvent::Deletion, None, &webhook);

        assert_eq!(payload["event"], "deletion");
        assert_eq!(payload["mailbox"], "test");
        assert_eq!(payload["webhook_id"], webhook.id);
        assert!(payload["email"].is_null());
        assert!(payload["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_webhook_payload_with_email() {
        let webhook = Webhook::new(
            "test".to_string(),
            "http://localhost:3009".to_string(),
            vec![WebhookEvent::Arrival],
        );

        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );

        let storage = Arc::new(crate::storage::sqlite::SqliteBackend::new("sqlite::memory:").await.unwrap());
        let trigger = WebhookTrigger::new(storage);
        let payload = trigger.create_webhook_payload(&WebhookEvent::Arrival, Some(&email), &webhook);

        assert_eq!(payload["event"], "arrival");
        assert_eq!(payload["mailbox"], "test");
        assert_eq!(payload["webhook_id"], webhook.id);
        assert!(payload["email"].is_object());
        assert_eq!(payload["email"]["id"], email.id);
        assert_eq!(payload["email"]["subject"], "Test Subject");
        assert!(payload["timestamp"].is_string());
    }
}
