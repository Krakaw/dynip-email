pub mod models;
pub mod sqlite;

use anyhow::Result;
use async_trait::async_trait;
use models::{Email, Webhook, WebhookEvent};

/// Trait defining the storage backend interface
/// This allows swapping storage implementations (SQLite, PostgreSQL, Redis, etc.)
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store a new email
    async fn store_email(&self, email: Email) -> Result<()>;

    /// Get all emails for a specific address
    async fn get_emails_for_address(&self, address: &str) -> Result<Vec<Email>>;

    /// Get a specific email by its ID
    async fn get_email_by_id(&self, id: &str) -> Result<Option<Email>>;

    /// Delete old emails and return details of deleted emails
    async fn delete_old_emails_with_details(&self, hours: i64) -> Result<Vec<(String, String)>>;

    /// Create a new webhook
    async fn create_webhook(&self, webhook: Webhook) -> Result<()>;

    /// Get all webhooks for a specific mailbox
    async fn get_webhooks_for_mailbox(&self, address: &str) -> Result<Vec<Webhook>>;

    /// Get a specific webhook by its ID
    async fn get_webhook_by_id(&self, id: &str) -> Result<Option<Webhook>>;

    /// Update an existing webhook
    async fn update_webhook(&self, webhook: Webhook) -> Result<()>;

    /// Delete a webhook by its ID
    async fn delete_webhook(&self, id: &str) -> Result<()>;

    /// Get active webhooks for a specific event and mailbox
    async fn get_active_webhooks_for_event(&self, address: &str, event: WebhookEvent) -> Result<Vec<Webhook>>;
}
