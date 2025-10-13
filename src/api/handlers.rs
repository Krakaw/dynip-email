use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use serde::Deserialize;

use crate::storage::{StorageBackend, models::{Webhook, WebhookEvent}};
use crate::webhooks::WebhookTrigger;
use std::sync::Arc;

/// Shared application configuration
#[derive(Clone)]
pub struct AppConfig {
    pub domain_name: String,
}

impl AppConfig {
    /// Normalize an email address by appending domain if not present
    pub fn normalize_address(&self, input: &str) -> String {
        let input = input.trim();

        // If it already contains @, use as-is
        if input.contains('@') {
            input.to_string()
        } else {
            // Append the server domain
            format!("{}@{}", input, self.domain_name)
        }
    }
}

/// Get all emails for a specific address
pub async fn get_emails_for_address(
    Path(address): Path<String>,
    State((storage, config)): State<(Arc<dyn StorageBackend>, AppConfig)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Normalize the address (append domain if not present)
    let normalized_address = config.normalize_address(&address);

    match storage.get_emails_for_address(&normalized_address).await {
        Ok(emails) => Ok(Json(json!({ "emails": emails }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch emails: {}", e),
        )),
    }
}

/// Get a specific email by ID
pub async fn get_email_by_id(
    Path(id): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    match storage.get_email_by_id(&id).await {
        Ok(Some(email)) => Ok(Json(json!(email))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Email not found".to_string())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch email: {}", e),
        )),
    }
}

/// Create webhook request
#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub mailbox_address: String,
    pub webhook_url: String,
    pub events: Vec<String>,
}

/// Update webhook request
#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub mailbox_address: Option<String>,
    pub webhook_url: Option<String>,
    pub events: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

/// Create a new webhook
pub async fn create_webhook(
    State(storage): State<Arc<dyn StorageBackend>>,
    Json(request): Json<CreateWebhookRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Parse events
    let events: Result<Vec<WebhookEvent>, _> = request.events
        .into_iter()
        .map(|s| WebhookEvent::from_str(&s).ok_or_else(|| format!("Invalid event: {}", s)))
        .collect();

    let events = match events {
        Ok(events) => events,
        Err(e) => return Err((StatusCode::BAD_REQUEST, e)),
    };

    // Validate and normalize webhook URL
    let webhook_url = if request.webhook_url.starts_with("http://") || request.webhook_url.starts_with("https://") {
        request.webhook_url
    } else {
        format!("http://{}", request.webhook_url)
    };

    // Extract mailbox name without domain for webhook storage
    let mailbox_name = request.mailbox_address.split('@').next().unwrap_or(&request.mailbox_address);

    let webhook = Webhook::new(
        mailbox_name.to_string(),
        webhook_url,
        events,
    );

    match storage.create_webhook(webhook.clone()).await {
        Ok(_) => Ok(Json(json!(webhook))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create webhook: {}", e),
        )),
    }
}

/// Get webhooks for a mailbox
pub async fn get_webhooks_for_mailbox(
    Path(address): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Extract mailbox name without domain for webhook lookup
    let mailbox_name = address.split('@').next().unwrap_or(&address);
    match storage.get_webhooks_for_mailbox(mailbox_name).await {
        Ok(webhooks) => Ok(Json(json!({ "webhooks": webhooks }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch webhooks: {}", e),
        )),
    }
}

/// Get a specific webhook by ID
pub async fn get_webhook_by_id(
    Path(id): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    match storage.get_webhook_by_id(&id).await {
        Ok(Some(webhook)) => Ok(Json(json!(webhook))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Webhook not found".to_string())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch webhook: {}", e),
        )),
    }
}

/// Update a webhook
pub async fn update_webhook(
    Path(id): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
    Json(request): Json<UpdateWebhookRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Get existing webhook
    let mut webhook = match storage.get_webhook_by_id(&id).await {
        Ok(Some(webhook)) => webhook,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Webhook not found".to_string())),
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch webhook: {}", e),
        )),
    };

    // Update fields if provided
    if let Some(mailbox_address) = request.mailbox_address {
        webhook.mailbox_address = mailbox_address;
    }
    if let Some(webhook_url) = request.webhook_url {
        // Normalize URL
        webhook.webhook_url = if webhook_url.starts_with("http://") || webhook_url.starts_with("https://") {
            webhook_url
        } else {
            format!("http://{}", webhook_url)
        };
    }
    if let Some(events) = request.events {
        let parsed_events: Result<Vec<WebhookEvent>, _> = events
            .into_iter()
            .map(|s| WebhookEvent::from_str(&s).ok_or_else(|| format!("Invalid event: {}", s)))
            .collect();

        match parsed_events {
            Ok(events) => webhook.events = events,
            Err(e) => return Err((StatusCode::BAD_REQUEST, e)),
        }
    }
    if let Some(enabled) = request.enabled {
        webhook.enabled = enabled;
    }

    match storage.update_webhook(webhook.clone()).await {
        Ok(_) => Ok(Json(json!(webhook))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update webhook: {}", e),
        )),
    }
}

/// Delete a webhook
pub async fn delete_webhook(
    Path(id): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    match storage.delete_webhook(&id).await {
        Ok(_) => Ok(Json(json!({ "message": "Webhook deleted successfully" }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete webhook: {}", e),
        )),
    }
}

/// Test a webhook
pub async fn test_webhook(
    Path(id): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let webhook = match storage.get_webhook_by_id(&id).await {
        Ok(Some(webhook)) => webhook,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Webhook not found".to_string())),
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch webhook: {}", e),
        )),
    };

    let webhook_trigger = WebhookTrigger::new(storage);
    match webhook_trigger.test_webhook(&webhook).await {
        Ok(success) => Ok(Json(json!({ "success": success }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to test webhook: {}", e),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_normalize_address() {
        let config = AppConfig {
            domain_name: "example.com".to_string(),
        };

        // Test normalization of address without @
        assert_eq!(config.normalize_address("user"), "user@example.com");

        // Test address with @ should remain unchanged
        assert_eq!(config.normalize_address("user@test.com"), "user@test.com");

        // Test address with @ and domain should remain unchanged
        assert_eq!(
            config.normalize_address("user@example.com"),
            "user@example.com"
        );

        // Test trimming whitespace
        assert_eq!(config.normalize_address("  user  "), "user@example.com");

        // Test empty string
        assert_eq!(config.normalize_address(""), "@example.com");
    }

    #[test]
    fn test_app_config_with_different_domain() {
        let config = AppConfig {
            domain_name: "test.local".to_string(),
        };

        // Test normalization with different domain
        assert_eq!(config.normalize_address("user"), "user@test.local");
        assert_eq!(
            config.normalize_address("user@example.com"),
            "user@example.com"
        );
        assert_eq!(
            config.normalize_address("user@test.local"),
            "user@test.local"
        );
    }

    #[test]
    fn test_app_config_edge_cases() {
        let config = AppConfig {
            domain_name: "example.com".to_string(),
        };

        // Test with @ in the middle
        assert_eq!(config.normalize_address("user@domain"), "user@domain");

        // Test with multiple @ symbols
        assert_eq!(config.normalize_address("user@@domain"), "user@@domain");

        // Test with just @
        assert_eq!(config.normalize_address("@"), "@");

        // Test with domain only
        assert_eq!(config.normalize_address("@example.com"), "@example.com");
    }
}
