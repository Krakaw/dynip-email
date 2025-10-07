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

