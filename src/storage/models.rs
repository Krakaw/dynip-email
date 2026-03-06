use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Email attachment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attachment {
    /// Filename of the attachment
    pub filename: String,

    /// MIME type of the attachment
    pub content_type: String,

    /// Size of the attachment in bytes
    pub size: usize,

    /// Base64-encoded content of the attachment
    pub content: String,
}

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

    /// Attachments
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

impl Email {
    /// Create a new email with generated UUID
    pub fn new(
        to: String,
        from: String,
        subject: String,
        body: String,
        raw: Option<String>,
        attachments: Vec<Attachment>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            to,
            from,
            subject,
            body,
            timestamp: Utc::now(),
            raw,
            attachments,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_attachment_creation() {
        let attachment = Attachment {
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 100,
            content: "dGVzdCBjb250ZW50".to_string(), // base64 encoded "test content"
        };

        assert_eq!(attachment.filename, "test.txt");
        assert_eq!(attachment.content_type, "text/plain");
        assert_eq!(attachment.size, 100);
        assert_eq!(attachment.content, "dGVzdCBjb250ZW50");
    }

    #[test]
    fn test_email_creation() {
        let attachments = vec![Attachment {
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 100,
            content: "dGVzdCBjb250ZW50".to_string(),
        }];

        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body content".to_string(),
            Some("raw email content".to_string()),
            attachments.clone(),
        );

        assert_eq!(email.to, "test@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "Test Subject");
        assert_eq!(email.body, "Test body content");
        assert_eq!(email.raw, Some("raw email content".to_string()));
        assert_eq!(email.attachments, attachments);

        // Check that ID is generated (UUID format)
        assert!(!email.id.is_empty());
        assert!(email.id.len() > 10);

        // Check that timestamp is recent
        let now = Utc::now();
        let diff = now.signed_duration_since(email.timestamp);
        assert!(diff.num_seconds() < 5); // Should be within 5 seconds
    }

    #[test]
    fn test_email_creation_without_raw() {
        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body content".to_string(),
            None,
            vec![],
        );

        assert_eq!(email.raw, None);
        assert!(email.attachments.is_empty());
    }

    #[test]
    fn test_email_creation_with_multiple_attachments() {
        let attachments = vec![
            Attachment {
                filename: "file1.txt".to_string(),
                content_type: "text/plain".to_string(),
                size: 50,
                content: "Y29udGVudDE=".to_string(),
            },
            Attachment {
                filename: "file2.pdf".to_string(),
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

        assert_eq!(email.attachments.len(), 2);
        assert_eq!(email.attachments[0].filename, "file1.txt");
        assert_eq!(email.attachments[1].filename, "file2.pdf");
    }

    #[test]
    fn test_email_serialization() {
        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body content".to_string(),
            Some("raw email content".to_string()),
            vec![],
        );

        // Test JSON serialization
        let json = serde_json::to_string(&email).unwrap();
        assert!(json.contains("test@example.com"));
        assert!(json.contains("Test Subject"));

        // Test JSON deserialization
        let deserialized: Email = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.to, email.to);
        assert_eq!(deserialized.from, email.from);
        assert_eq!(deserialized.subject, email.subject);
        assert_eq!(deserialized.body, email.body);
        assert_eq!(deserialized.raw, email.raw);
        assert_eq!(deserialized.attachments.len(), email.attachments.len());
    }

    #[test]
    fn test_attachment_serialization() {
        let attachment = Attachment {
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 100,
            content: "dGVzdCBjb250ZW50".to_string(),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&attachment).unwrap();
        assert!(json.contains("test.txt"));
        assert!(json.contains("text/plain"));

        // Test JSON deserialization
        let deserialized: Attachment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.filename, attachment.filename);
        assert_eq!(deserialized.content_type, attachment.content_type);
        assert_eq!(deserialized.size, attachment.size);
        assert_eq!(deserialized.content, attachment.content);
    }

    #[test]
    fn test_sent_email_creation() {
        let sent = SentEmail::new(
            "sender@example.com".to_string(),
            "recipient@example.com".to_string(),
            "Test Subject".to_string(),
            "Hello, world!".to_string(),
            None,
            "<msg123@example.com>".to_string(),
        );

        assert_eq!(sent.from, "sender@example.com");
        assert_eq!(sent.to, "recipient@example.com");
        assert_eq!(sent.subject, "Test Subject");
        assert_eq!(sent.body_text, "Hello, world!");
        assert!(sent.body_html.is_none());
        assert_eq!(sent.message_id, "<msg123@example.com>");
        assert!(!sent.id.is_empty());

        let now = Utc::now();
        let diff = now.signed_duration_since(sent.timestamp);
        assert!(diff.num_seconds() < 5);
    }

    #[test]
    fn test_sent_email_with_html() {
        let sent = SentEmail::new(
            "sender@example.com".to_string(),
            "recipient@example.com".to_string(),
            "HTML Email".to_string(),
            "Plain text".to_string(),
            Some("<h1>Hello</h1>".to_string()),
            "<msg456@example.com>".to_string(),
        );

        assert_eq!(sent.body_html, Some("<h1>Hello</h1>".to_string()));
    }

    #[test]
    fn test_sent_email_serialization() {
        let sent = SentEmail::new(
            "sender@example.com".to_string(),
            "recipient@example.com".to_string(),
            "Test".to_string(),
            "Body".to_string(),
            None,
            "<msg@example.com>".to_string(),
        );

        let json = serde_json::to_string(&sent).unwrap();
        assert!(json.contains("sender@example.com"));
        assert!(json.contains("recipient@example.com"));

        let deserialized: SentEmail = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.from, sent.from);
        assert_eq!(deserialized.to, sent.to);
        assert_eq!(deserialized.subject, sent.subject);
    }
}

/// Sent email model representing an outbound email
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentEmail {
    pub id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub message_id: String,
}

impl SentEmail {
    pub fn new(
        from: String,
        to: String,
        subject: String,
        body_text: String,
        body_html: Option<String>,
        message_id: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from,
            to,
            subject,
            body_text,
            body_html,
            timestamp: Utc::now(),
            message_id,
        }
    }
}

/// Webhook event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WebhookEvent {
    Arrival,
    Deletion,
}

impl WebhookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            WebhookEvent::Arrival => "arrival",
            WebhookEvent::Deletion => "deletion",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "arrival" => Some(WebhookEvent::Arrival),
            "deletion" => Some(WebhookEvent::Deletion),
            _ => None,
        }
    }
}

/// Webhook configuration model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    /// Unique identifier for the webhook
    pub id: String,

    /// Email address this webhook is for (without domain)
    pub mailbox_address: String,

    /// Target webhook URL
    pub webhook_url: String,

    /// Subscribed events
    pub events: Vec<WebhookEvent>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Whether the webhook is enabled
    pub enabled: bool,
}

impl Webhook {
    /// Create a new webhook with generated UUID
    pub fn new(mailbox_address: String, webhook_url: String, events: Vec<WebhookEvent>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            mailbox_address,
            webhook_url,
            events,
            created_at: Utc::now(),
            enabled: true,
        }
    }
}

/// Mailbox model representing a protected mailbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mailbox {
    /// Mailbox address (unique identifier)
    pub address: String,

    /// Bcrypt password hash (None if not yet claimed)
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,

    /// When the mailbox was first claimed
    pub created_at: DateTime<Utc>,

    /// Whether the mailbox is locked (has a password)
    pub is_locked: bool,
}

#[allow(dead_code)]
impl Mailbox {
    /// Create a new unclaimed mailbox
    pub fn new(address: String) -> Self {
        Self {
            address,
            password_hash: None,
            created_at: Utc::now(),
            is_locked: false,
        }
    }

    /// Create a mailbox with a password hash
    pub fn with_password(address: String, password_hash: String) -> Self {
        Self {
            address,
            password_hash: Some(password_hash),
            created_at: Utc::now(),
            is_locked: true,
        }
    }
}

/// User model for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique identifier for the user
    pub id: String,

    /// Email address (unique, used as username)
    pub email: String,

    /// Bcrypt password hash
    #[serde(skip_serializing)]
    pub password_hash: String,

    /// When the user was created
    pub created_at: DateTime<Utc>,
}

impl User {
    /// Create a new user with generated UUID
    pub fn new(email: String, password_hash: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            email,
            password_hash,
            created_at: Utc::now(),
        }
    }
}
