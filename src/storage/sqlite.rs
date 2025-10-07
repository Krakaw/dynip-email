use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tracing::{info, warn};

use super::{models::Email, StorageBackend};

/// SQLite implementation of StorageBackend
pub struct SqliteBackend {
    pool: SqlitePool,
}

impl SqliteBackend {
    /// Create a new SQLite backend with the given database URL
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to SQLite database: {}", database_url);
        
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
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
                raw TEXT
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
        
        info!("SQLite database initialized successfully");
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl StorageBackend for SqliteBackend {
    async fn store_email(&self, email: Email) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO emails (id, to_address, from_address, subject, body, timestamp, raw)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&email.id)
        .bind(&email.to)
        .bind(&email.from)
        .bind(&email.subject)
        .bind(&email.body)
        .bind(email.timestamp.to_rfc3339())
        .bind(&email.raw)
        .execute(&self.pool)
        .await?;
        
        info!("Stored email {} for address {}", email.id, email.to);
        Ok(())
    }
    
    async fn get_emails_for_address(&self, address: &str) -> Result<Vec<Email>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, Option<String>)>(
            r#"
            SELECT id, to_address, from_address, subject, body, timestamp, raw
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
            .map(|(id, to, from, subject, body, timestamp, raw)| {
                let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc);
                
                Email {
                    id,
                    to,
                    from,
                    subject,
                    body,
                    timestamp,
                    raw,
                }
            })
            .collect();
        
        Ok(emails)
    }
    
    async fn get_email_by_id(&self, id: &str) -> Result<Option<Email>> {
        let row = sqlx::query_as::<_, (String, String, String, String, String, String, Option<String>)>(
            r#"
            SELECT id, to_address, from_address, subject, body, timestamp, raw
            FROM emails
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.map(|(id, to, from, subject, body, timestamp, raw)| {
            let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc);
            
            Email {
                id,
                to,
                from,
                subject,
                body,
                timestamp,
                raw,
            }
        }))
    }
    
    async fn delete_old_emails(&self, hours: i64) -> Result<usize> {
        let cutoff = Utc::now() - Duration::hours(hours);
        let cutoff_str = cutoff.to_rfc3339();
        
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
            warn!("Deleted {} old emails (older than {} hours)", deleted, hours);
        }
        
        Ok(deleted)
    }
}

