use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use crate::storage::StorageBackend;
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
