use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Email model representing a stored email
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    /// Unique identifier for the email
    pub id: String,
    
    /// Recipient email address
    pub to: String,
    
    /// Sender email address
    pub from: String,
    
    /// Email subject
    pub subject: String,
    
    /// Email body (can be text or HTML)
    pub body: String,
    
    /// Timestamp when email was received
    pub timestamp: DateTime<Utc>,
    
    /// Optional raw email data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

impl Email {
    /// Create a new email with generated UUID
    pub fn new(to: String, from: String, subject: String, body: String, raw: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            to,
            from,
            subject,
            body,
            timestamp: Utc::now(),
            raw,
        }
    }
}

