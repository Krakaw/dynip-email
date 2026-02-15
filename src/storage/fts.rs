use serde::{Deserialize, Serialize};

/// Search result with highlighted snippets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Email ID
    pub id: String,
    /// Recipient email address
    pub to: String,
    /// Sender email address
    pub from: String,
    /// Email subject
    pub subject: String,
    /// Highlighted snippet from body
    pub snippet: String,
    /// Timestamp when email was received
    pub timestamp: String,
    /// Search relevance rank
    pub rank: f64,
}

/// FTS5 search query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Search query string (FTS5 syntax supported)
    pub query: String,
    /// Maximum number of results to return
    pub limit: Option<i64>,
    /// Search only in specific mailbox (optional)
    pub mailbox: Option<String>,
}

impl SearchQuery {
    /// Create a new search query
    pub fn new(query: String) -> Self {
        Self {
            query,
            limit: Some(50),
            mailbox: None,
        }
    }

    /// Set the result limit
    pub fn with_limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the mailbox filter
    pub fn with_mailbox(mut self, mailbox: String) -> Self {
        self.mailbox = Some(mailbox);
        self
    }
}
