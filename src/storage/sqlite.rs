use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use tracing::{info, warn};

use super::{
    models::{Email, Mailbox, Webhook, WebhookEvent},
    StorageBackend,
};

/// SQLite implementation of StorageBackend
pub struct SqliteBackend {
    pool: SqlitePool,
}

impl SqliteBackend {
    /// Create a new SQLite backend with the given database URL
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to SQLite database: {}", database_url);

        // Parse connection options and enable create_if_missing
        let connect_options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await?;

        // Run migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS emails (
                id TEXT PRIMARY KEY,
                to_address TEXT NOT NULL,
                from_address TEXT NOT NULL,
                subject TEXT NOT NULL,
                body TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                raw TEXT,
                attachments TEXT
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Create index on to_address for faster queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_to_address ON emails(to_address)
            "#,
        )
        .execute(&pool)
        .await?;

        // Create index on timestamp for cleanup queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_timestamp ON emails(timestamp)
            "#,
        )
        .execute(&pool)
        .await?;

        // Create webhooks table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS webhooks (
                id TEXT PRIMARY KEY,
                mailbox_address TEXT NOT NULL,
                webhook_url TEXT NOT NULL,
                events TEXT NOT NULL,
                created_at TEXT NOT NULL,
                enabled BOOLEAN DEFAULT 1
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Create index on mailbox_address for faster webhook queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_webhooks_mailbox ON webhooks(mailbox_address)
            "#,
        )
        .execute(&pool)
        .await?;

        // Create mailboxes table for password protection
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS mailboxes (
                address TEXT PRIMARY KEY,
                password_hash TEXT,
                created_at TEXT NOT NULL,
                is_locked BOOLEAN DEFAULT 0
            )
            "#,
        )
        .execute(&pool)
        .await?;

        info!("SQLite database initialized successfully");

        Ok(Self { pool })
    }
}

#[async_trait]
impl StorageBackend for SqliteBackend {
    async fn store_email(&self, email: Email) -> Result<()> {
        // Serialize attachments to JSON
        let attachments_json = serde_json::to_string(&email.attachments)?;

        sqlx::query(
            r#"
            INSERT INTO emails (id, to_address, from_address, subject, body, timestamp, raw, attachments)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&email.id)
        .bind(&email.to)
        .bind(&email.from)
        .bind(&email.subject)
        .bind(&email.body)
        .bind(email.timestamp.to_rfc3339())
        .bind(&email.raw)
        .bind(&attachments_json)
        .execute(&self.pool)
        .await?;

        info!(
            "Stored email {} for address {} with {} attachments",
            email.id,
            email.to,
            email.attachments.len()
        );
        Ok(())
    }

    async fn get_emails_for_address(&self, address: &str) -> Result<Vec<Email>> {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            SELECT id, to_address, from_address, subject, body, timestamp, raw, attachments
            FROM emails
            WHERE to_address = ?
            ORDER BY timestamp DESC
            "#,
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;

        let emails = rows
            .into_iter()
            .map(
                |(id, to, from, subject, body, timestamp, raw, attachments_json)| {
                    let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc);

                    // Deserialize attachments from JSON
                    let attachments = attachments_json
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .unwrap_or_default();

                    Email {
                        id,
                        to,
                        from,
                        subject,
                        body,
                        timestamp,
                        raw,
                        attachments,
                    }
                },
            )
            .collect();

        Ok(emails)
    }

    async fn get_email_by_id(&self, id: &str) -> Result<Option<Email>> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            SELECT id, to_address, from_address, subject, body, timestamp, raw, attachments
            FROM emails
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(id, to, from, subject, body, timestamp, raw, attachments_json)| {
                let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc);

                // Deserialize attachments from JSON
                let attachments = attachments_json
                    .and_then(|json| serde_json::from_str(&json).ok())
                    .unwrap_or_default();

                Email {
                    id,
                    to,
                    from,
                    subject,
                    body,
                    timestamp,
                    raw,
                    attachments,
                }
            },
        ))
    }

    async fn delete_email(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM emails WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_old_emails_with_details(&self, hours: i64) -> Result<Vec<(String, String)>> {
        let cutoff = Utc::now() - Duration::hours(hours);
        let cutoff_str = cutoff.to_rfc3339();

        // First, get the IDs and addresses of emails to be deleted
        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT id, to_address
            FROM emails
            WHERE timestamp < ?
            "#,
        )
        .bind(&cutoff_str)
        .fetch_all(&self.pool)
        .await?;

        let deleted_emails = rows.clone();

        // Then delete them
        let result = sqlx::query(
            r#"
            DELETE FROM emails
            WHERE timestamp < ?
            "#,
        )
        .bind(cutoff_str)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected() as usize;
        if deleted > 0 {
            warn!(
                "Deleted {} old emails (older than {} hours)",
                deleted, hours
            );
        }

        Ok(deleted_emails)
    }

    async fn create_webhook(&self, webhook: Webhook) -> Result<()> {
        // Serialize events to JSON
        let events_json = serde_json::to_string(&webhook.events)?;

        sqlx::query(
            r#"
            INSERT INTO webhooks (id, mailbox_address, webhook_url, events, created_at, enabled)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&webhook.id)
        .bind(&webhook.mailbox_address)
        .bind(&webhook.webhook_url)
        .bind(&events_json)
        .bind(webhook.created_at.to_rfc3339())
        .bind(webhook.enabled)
        .execute(&self.pool)
        .await?;

        info!(
            "Created webhook {} for mailbox {} with {} events",
            webhook.id,
            webhook.mailbox_address,
            webhook.events.len()
        );
        Ok(())
    }

    async fn get_webhooks_for_mailbox(&self, address: &str) -> Result<Vec<Webhook>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
            r#"
            SELECT id, mailbox_address, webhook_url, events, created_at, enabled
            FROM webhooks
            WHERE mailbox_address = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;

        let webhooks = rows
            .into_iter()
            .map(
                |(id, mailbox_address, webhook_url, events_json, created_at, enabled)| {
                    let created_at = DateTime::parse_from_rfc3339(&created_at)
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc);

                    // Deserialize events from JSON
                    let events = serde_json::from_str(&events_json).unwrap_or_default();

                    Webhook {
                        id,
                        mailbox_address,
                        webhook_url,
                        events,
                        created_at,
                        enabled,
                    }
                },
            )
            .collect();

        Ok(webhooks)
    }

    async fn get_webhook_by_id(&self, id: &str) -> Result<Option<Webhook>> {
        let row = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
            r#"
            SELECT id, mailbox_address, webhook_url, events, created_at, enabled
            FROM webhooks
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(id, mailbox_address, webhook_url, events_json, created_at, enabled)| {
                let created_at = DateTime::parse_from_rfc3339(&created_at)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc);

                // Deserialize events from JSON
                let events = serde_json::from_str(&events_json).unwrap_or_default();

                Webhook {
                    id,
                    mailbox_address,
                    webhook_url,
                    events,
                    created_at,
                    enabled,
                }
            },
        ))
    }

    async fn update_webhook(&self, webhook: Webhook) -> Result<()> {
        // Serialize events to JSON
        let events_json = serde_json::to_string(&webhook.events)?;

        sqlx::query(
            r#"
            UPDATE webhooks
            SET mailbox_address = ?, webhook_url = ?, events = ?, enabled = ?
            WHERE id = ?
            "#,
        )
        .bind(&webhook.mailbox_address)
        .bind(&webhook.webhook_url)
        .bind(&events_json)
        .bind(webhook.enabled)
        .bind(&webhook.id)
        .execute(&self.pool)
        .await?;

        info!("Updated webhook {}", webhook.id);
        Ok(())
    }

    async fn delete_webhook(&self, id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM webhooks
            WHERE id = ?
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        info!("Deleted webhook {}", id);
        Ok(())
    }

    async fn get_active_webhooks_for_event(
        &self,
        address: &str,
        event: WebhookEvent,
    ) -> Result<Vec<Webhook>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
            r#"
            SELECT id, mailbox_address, webhook_url, events, created_at, enabled
            FROM webhooks
            WHERE mailbox_address = ? AND enabled = 1
            "#,
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;

        let webhooks = rows
            .into_iter()
            .map(
                |(id, mailbox_address, webhook_url, events_json, created_at, enabled)| {
                    let created_at = DateTime::parse_from_rfc3339(&created_at)
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc);

                    // Deserialize events from JSON
                    let events = serde_json::from_str(&events_json).unwrap_or_default();

                    Webhook {
                        id,
                        mailbox_address,
                        webhook_url,
                        events,
                        created_at,
                        enabled,
                    }
                },
            )
            .filter(|webhook| webhook.events.contains(&event))
            .collect();

        Ok(webhooks)
    }

    async fn get_mailbox(&self, address: &str) -> Result<Option<Mailbox>> {
        let row = sqlx::query_as::<_, (String, Option<String>, String, bool)>(
            r#"
            SELECT address, password_hash, created_at, is_locked
            FROM mailboxes
            WHERE address = ?
            "#,
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(address, password_hash, created_at, is_locked)| {
            let created_at = DateTime::parse_from_rfc3339(&created_at)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc);

            Mailbox {
                address,
                password_hash,
                created_at,
                is_locked,
            }
        }))
    }

    async fn set_mailbox_password(&self, address: &str, password_hash: String) -> Result<()> {
        // Check if mailbox exists
        let existing = self.get_mailbox(address).await?;

        if let Some(mailbox) = existing {
            // Mailbox already exists and is locked - don't allow changing password
            if mailbox.is_locked {
                return Err(anyhow::anyhow!(
                    "Mailbox is already locked with a password"
                ));
            }
            // Update existing unlocked mailbox
            sqlx::query(
                r#"
                UPDATE mailboxes
                SET password_hash = ?, is_locked = 1
                WHERE address = ?
                "#,
            )
            .bind(&password_hash)
            .bind(address)
            .execute(&self.pool)
            .await?;
        } else {
            // Create new mailbox
            sqlx::query(
                r#"
                INSERT INTO mailboxes (address, password_hash, created_at, is_locked)
                VALUES (?, ?, ?, 1)
                "#,
            )
            .bind(address)
            .bind(&password_hash)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        }

        info!("Set password for mailbox {}", address);
        Ok(())
    }

    async fn is_mailbox_locked(&self, address: &str) -> Result<bool> {
        let mailbox = self.get_mailbox(address).await?;
        Ok(mailbox.map(|m| m.is_locked).unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{Attachment, Email};
    use chrono::{Duration, Utc};

    async fn create_test_backend() -> SqliteBackend {
        // Use in-memory database for tests
        let database_url = "sqlite::memory:";
        SqliteBackend::new(database_url).await.unwrap()
    }

    #[tokio::test]
    async fn test_sqlite_backend_creation() {
        let _backend = create_test_backend().await;
        // If we get here without panicking, the backend was created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_store_and_retrieve_email() {
        let backend = create_test_backend().await;

        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body content".to_string(),
            Some("raw email content".to_string()),
            vec![],
        );

        // Store the email
        backend.store_email(email.clone()).await.unwrap();

        // Retrieve by address
        let emails = backend
            .get_emails_for_address("test@example.com")
            .await
            .unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].id, email.id);
        assert_eq!(emails[0].to, email.to);
        assert_eq!(emails[0].from, email.from);
        assert_eq!(emails[0].subject, email.subject);
        assert_eq!(emails[0].body, email.body);
        assert_eq!(emails[0].raw, email.raw);

        // Retrieve by ID
        let retrieved_email = backend.get_email_by_id(&email.id).await.unwrap();
        assert!(retrieved_email.is_some());
        let retrieved_email = retrieved_email.unwrap();
        assert_eq!(retrieved_email.id, email.id);
        assert_eq!(retrieved_email.to, email.to);
    }

    #[tokio::test]
    async fn test_store_email_with_attachments() {
        let backend = create_test_backend().await;

        let attachments = vec![
            Attachment {
                filename: "test.txt".to_string(),
                content_type: "text/plain".to_string(),
                size: 100,
                content: "dGVzdCBjb250ZW50".to_string(),
            },
            Attachment {
                filename: "test.pdf".to_string(),
                content_type: "application/pdf".to_string(),
                size: 200,
                content: "cGRmIGNvbnRlbnQ=".to_string(),
            },
        ];

        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body content".to_string(),
            None,
            attachments.clone(),
        );

        // Store the email
        backend.store_email(email.clone()).await.unwrap();

        // Retrieve and verify attachments
        let emails = backend
            .get_emails_for_address("test@example.com")
            .await
            .unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].attachments.len(), 2);
        assert_eq!(emails[0].attachments[0].filename, "test.txt");
        assert_eq!(emails[0].attachments[1].filename, "test.pdf");
    }

    #[tokio::test]
    async fn test_get_emails_for_nonexistent_address() {
        let backend = create_test_backend().await;

        let emails = backend
            .get_emails_for_address("nonexistent@example.com")
            .await
            .unwrap();
        assert!(emails.is_empty());
    }

    #[tokio::test]
    async fn test_get_email_by_nonexistent_id() {
        let backend = create_test_backend().await;

        let email = backend.get_email_by_id("nonexistent-id").await.unwrap();
        assert!(email.is_none());
    }

    #[tokio::test]
    async fn test_multiple_emails_for_same_address() {
        let backend = create_test_backend().await;

        let email1 = Email::new(
            "test@example.com".to_string(),
            "sender1@example.com".to_string(),
            "First Subject".to_string(),
            "First body".to_string(),
            None,
            vec![],
        );

        let email2 = Email::new(
            "test@example.com".to_string(),
            "sender2@example.com".to_string(),
            "Second Subject".to_string(),
            "Second body".to_string(),
            None,
            vec![],
        );

        // Store both emails
        backend.store_email(email1.clone()).await.unwrap();
        backend.store_email(email2.clone()).await.unwrap();

        // Retrieve all emails for the address
        let emails = backend
            .get_emails_for_address("test@example.com")
            .await
            .unwrap();
        assert_eq!(emails.len(), 2);

        // Emails should be ordered by timestamp DESC (newest first)
        // Since we created them in quick succession, we can't guarantee order
        // but we can verify both are present
        let ids: Vec<String> = emails.iter().map(|e| e.id.clone()).collect();
        assert!(ids.contains(&email1.id));
        assert!(ids.contains(&email2.id));
    }

    #[tokio::test]
    async fn test_delete_old_emails() {
        let backend = create_test_backend().await;

        // Create an old email (simulate by manually setting timestamp)
        let mut old_email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Old Subject".to_string(),
            "Old body".to_string(),
            None,
            vec![],
        );
        old_email.timestamp = Utc::now() - Duration::hours(25); // 25 hours ago

        let new_email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "New Subject".to_string(),
            "New body".to_string(),
            None,
            vec![],
        );

        // Store both emails
        backend.store_email(old_email.clone()).await.unwrap();
        backend.store_email(new_email.clone()).await.unwrap();

        // Verify both emails exist
        let emails = backend
            .get_emails_for_address("test@example.com")
            .await
            .unwrap();
        assert_eq!(emails.len(), 2);

        // Delete emails older than 24 hours
        let deleted_details = backend.delete_old_emails_with_details(24).await.unwrap();
        assert_eq!(deleted_details.len(), 1);

        // Verify only the new email remains
        let emails = backend
            .get_emails_for_address("test@example.com")
            .await
            .unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].id, new_email.id);
    }

    #[tokio::test]
    async fn test_delete_old_emails_with_details() {
        let backend = create_test_backend().await;

        // Create an old email
        let mut old_email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Old Subject".to_string(),
            "Old body".to_string(),
            None,
            vec![],
        );
        old_email.timestamp = Utc::now() - Duration::hours(25); // 25 hours ago

        // Store the email
        backend.store_email(old_email.clone()).await.unwrap();

        // Delete emails older than 24 hours and get details
        let deleted_details = backend.delete_old_emails_with_details(24).await.unwrap();
        assert_eq!(deleted_details.len(), 1);
        assert_eq!(deleted_details[0].0, old_email.id);
        assert_eq!(deleted_details[0].1, old_email.to);
    }

    #[tokio::test]
    async fn test_delete_old_emails_no_old_emails() {
        let backend = create_test_backend().await;

        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Recent Subject".to_string(),
            "Recent body".to_string(),
            None,
            vec![],
        );

        // Store the email
        backend.store_email(email.clone()).await.unwrap();

        // Try to delete emails older than 24 hours (should delete none)
        let deleted_details = backend.delete_old_emails_with_details(24).await.unwrap();
        assert_eq!(deleted_details.len(), 0);

        // Verify the email still exists
        let emails = backend
            .get_emails_for_address("test@example.com")
            .await
            .unwrap();
        assert_eq!(emails.len(), 1);
    }

    #[tokio::test]
    async fn test_database_initialization() {
        // Use in-memory database for tests
        let database_url = "sqlite::memory:";

        // Create backend (this should initialize the database)
        let backend = SqliteBackend::new(database_url).await.unwrap();

        // Verify tables were created by trying to query them
        let emails = backend
            .get_emails_for_address("test@example.com")
            .await
            .unwrap();
        assert!(emails.is_empty()); // Should not panic, just return empty
    }
}
