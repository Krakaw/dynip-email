pub mod models;
pub mod sqlite;

use anyhow::Result;
use async_trait::async_trait;
use models::Email;

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

    /// Delete old emails (for cleanup/retention)
    async fn delete_old_emails(&self, hours: i64) -> Result<usize>;

    /// Delete old emails and return details of deleted emails
    async fn delete_old_emails_with_details(&self, hours: i64) -> Result<Vec<(String, String)>>;
}
