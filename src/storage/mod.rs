pub mod models;
pub mod sqlite;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use models::{Email, Mailbox, User, Webhook, WebhookEvent};

use crate::rate_limit::{RateLimit, RateLimitRequest};

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

    /// Delete a specific email by its ID
    async fn delete_email(&self, id: &str) -> Result<()>;

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
    async fn get_active_webhooks_for_event(
        &self,
        address: &str,
        event: WebhookEvent,
    ) -> Result<Vec<Webhook>>;

    /// Get mailbox by address
    async fn get_mailbox(&self, address: &str) -> Result<Option<Mailbox>>;

    /// Create or update a mailbox with password hash
    async fn set_mailbox_password(&self, address: &str, password_hash: String) -> Result<()>;

    /// Verify if a mailbox exists and is locked (has a password)
    async fn is_mailbox_locked(&self, address: &str) -> Result<bool>;

    /// Clear the password and unlock a mailbox
    async fn clear_mailbox_password(&self, address: &str) -> Result<()>;

    /// Verify a mailbox password
    async fn verify_mailbox_password(&self, address: &str, password: &str) -> Result<bool>;

    // User authentication methods

    /// Create a new user
    async fn create_user(&self, user: User) -> Result<()>;

    /// Get a user by email address
    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>>;

    /// Get a user by ID
    async fn get_user_by_id(&self, id: &str) -> Result<Option<User>>;

    /// Check if any users exist (for determining if registration should be open)
    async fn has_users(&self) -> Result<bool>;

    // Rate limiting methods

    /// Create a new rate limit
    async fn create_rate_limit(&self, rate_limit: RateLimit) -> Result<()>;

    /// Get a rate limit by mailbox address
    async fn get_rate_limit(&self, address: &str) -> Result<Option<RateLimit>>;

    /// Update an existing rate limit
    async fn update_rate_limit(&self, rate_limit: RateLimit) -> Result<()>;

    /// Delete a rate limit
    async fn delete_rate_limit(&self, address: &str) -> Result<()>;

    /// Record a rate limit request
    async fn record_rate_limit_request(&self, request: RateLimitRequest) -> Result<()>;

    /// Count requests since a given timestamp for a mailbox
    async fn count_requests_since(&self, address: &str, since: DateTime<Utc>) -> Result<u32>;

    /// Get the oldest request timestamp since a given time (for calculating retry-after)
    async fn get_oldest_request_since(
        &self,
        address: &str,
        since: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>>;

    /// Clean up old rate limit requests (optional, for maintenance)
    async fn cleanup_old_rate_limit_requests(&self, before: DateTime<Utc>) -> Result<u64>;
}
