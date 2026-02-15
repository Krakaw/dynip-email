use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

use crate::rate_limit::RateLimit;
use crate::storage::StorageBackend;

/// Request to create or update a rate limit
#[derive(Debug, Deserialize)]
pub struct SetRateLimitRequest {
    pub requests_per_hour: u32,
    pub requests_per_day: u32,
}

/// Response containing rate limit information
#[derive(Debug, Serialize)]
pub struct RateLimitResponse {
    pub mailbox_address: String,
    pub requests_per_hour: u32,
    pub requests_per_day: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<RateLimit> for RateLimitResponse {
    fn from(limit: RateLimit) -> Self {
        Self {
            mailbox_address: limit.mailbox_address,
            requests_per_hour: limit.requests_per_hour,
            requests_per_day: limit.requests_per_day,
            created_at: limit.created_at.to_rfc3339(),
            updated_at: limit.updated_at.to_rfc3339(),
        }
    }
}

/// Get rate limit for a specific mailbox
pub async fn get_rate_limit(
    Path(address): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    match storage.get_rate_limit(&address).await {
        Ok(Some(limit)) => Ok(Json(json!(RateLimitResponse::from(limit)))),
        Ok(None) => {
            // Return default rate limit if none exists
            let default_limit = RateLimit::new(address);
            Ok(Json(json!(RateLimitResponse::from(default_limit))))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch rate limit: {}", e),
        )),
    }
}

/// Set or update rate limit for a specific mailbox
pub async fn set_rate_limit(
    Path(address): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
    Json(request): Json<SetRateLimitRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Validate inputs
    if request.requests_per_hour == 0 || request.requests_per_day == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Rate limits must be greater than zero".to_string(),
        ));
    }

    if request.requests_per_hour > request.requests_per_day {
        return Err((
            StatusCode::BAD_REQUEST,
            "Hourly limit cannot exceed daily limit".to_string(),
        ));
    }

    // Check if rate limit exists
    let existing = storage.get_rate_limit(&address).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to check existing rate limit: {}", e),
        )
    })?;

    if let Some(mut limit) = existing {
        // Update existing rate limit
        limit.requests_per_hour = request.requests_per_hour;
        limit.requests_per_day = request.requests_per_day;
        limit.updated_at = chrono::Utc::now();

        storage
            .update_rate_limit(limit.clone())
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to update rate limit: {}", e),
                )
            })?;

        info!(
            "Updated rate limit for {}: {}/hr, {}/day",
            address, request.requests_per_hour, request.requests_per_day
        );

        Ok(Json(json!({
            "message": "Rate limit updated successfully",
            "rate_limit": RateLimitResponse::from(limit)
        })))
    } else {
        // Create new rate limit
        let limit = RateLimit::with_limits(
            address.clone(),
            request.requests_per_hour,
            request.requests_per_day,
        );

        storage
            .create_rate_limit(limit.clone())
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create rate limit: {}", e),
                )
            })?;

        info!(
            "Created rate limit for {}: {}/hr, {}/day",
            address, request.requests_per_hour, request.requests_per_day
        );

        Ok(Json(json!({
            "message": "Rate limit created successfully",
            "rate_limit": RateLimitResponse::from(limit)
        })))
    }
}

/// Delete rate limit for a specific mailbox (revert to defaults)
pub async fn delete_rate_limit(
    Path(address): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    storage.delete_rate_limit(&address).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete rate limit: {}", e),
        )
    })?;

    info!("Deleted rate limit for {} (reverted to defaults)", address);

    Ok(Json(json!({
        "message": "Rate limit deleted successfully (reverted to defaults)"
    })))
}

/// Get rate limit stats for a mailbox (current usage)
pub async fn get_rate_limit_stats(
    Path(address): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Get rate limit
    let rate_limit = match storage.get_rate_limit(&address).await {
        Ok(Some(limit)) => limit,
        Ok(None) => RateLimit::new(address.clone()),
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch rate limit: {}", e),
            ))
        }
    };

    // Get current usage
    let now = chrono::Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);
    let one_day_ago = now - chrono::Duration::days(1);

    let hourly_count = storage
        .count_requests_since(&address, one_hour_ago)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to count hourly requests: {}", e),
            )
        })?;

    let daily_count = storage
        .count_requests_since(&address, one_day_ago)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to count daily requests: {}", e),
            )
        })?;

    Ok(Json(json!({
        "mailbox_address": address,
        "rate_limit": {
            "requests_per_hour": rate_limit.requests_per_hour,
            "requests_per_day": rate_limit.requests_per_day
        },
        "current_usage": {
            "hourly": {
                "count": hourly_count,
                "limit": rate_limit.requests_per_hour,
                "remaining": rate_limit.requests_per_hour.saturating_sub(hourly_count),
                "percentage": (hourly_count as f64 / rate_limit.requests_per_hour as f64 * 100.0).min(100.0)
            },
            "daily": {
                "count": daily_count,
                "limit": rate_limit.requests_per_day,
                "remaining": rate_limit.requests_per_day.saturating_sub(daily_count),
                "percentage": (daily_count as f64 / rate_limit.requests_per_day as f64 * 100.0).min(100.0)
            }
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::SqliteBackend;

    async fn create_test_storage() -> Arc<dyn StorageBackend> {
        Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap())
    }

    #[tokio::test]
    async fn test_get_rate_limit_default() {
        let storage = create_test_storage().await;
        let address = "test@example.com".to_string();

        let result = get_rate_limit(Path(address), State(storage)).await;
        assert!(result.is_ok());

        let json = result.unwrap().0;
        assert_eq!(json["requests_per_hour"], 100);
        assert_eq!(json["requests_per_day"], 1000);
    }

    #[tokio::test]
    async fn test_set_and_get_rate_limit() {
        let storage = create_test_storage().await;
        let address = "test@example.com".to_string();

        let request = SetRateLimitRequest {
            requests_per_hour: 50,
            requests_per_day: 500,
        };

        let set_result =
            set_rate_limit(Path(address.clone()), State(storage.clone()), Json(request)).await;
        assert!(set_result.is_ok());

        let get_result = get_rate_limit(Path(address), State(storage)).await;
        assert!(get_result.is_ok());

        let json = get_result.unwrap().0;
        assert_eq!(json["requests_per_hour"], 50);
        assert_eq!(json["requests_per_day"], 500);
    }

    #[tokio::test]
    async fn test_set_rate_limit_validation() {
        let storage = create_test_storage().await;
        let address = "test@example.com".to_string();

        // Test zero hourly limit
        let request = SetRateLimitRequest {
            requests_per_hour: 0,
            requests_per_day: 500,
        };

        let result =
            set_rate_limit(Path(address.clone()), State(storage.clone()), Json(request)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::BAD_REQUEST);

        // Test hourly > daily
        let request = SetRateLimitRequest {
            requests_per_hour: 1000,
            requests_per_day: 500,
        };

        let result = set_rate_limit(Path(address), State(storage), Json(request)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_delete_rate_limit() {
        let storage = create_test_storage().await;
        let address = "test@example.com".to_string();

        // Create a rate limit
        let request = SetRateLimitRequest {
            requests_per_hour: 50,
            requests_per_day: 500,
        };

        set_rate_limit(Path(address.clone()), State(storage.clone()), Json(request))
            .await
            .unwrap();

        // Delete it
        let delete_result = delete_rate_limit(Path(address.clone()), State(storage.clone())).await;
        assert!(delete_result.is_ok());

        // Verify it's gone (returns default)
        let get_result = get_rate_limit(Path(address), State(storage)).await;
        let json = get_result.unwrap().0;
        assert_eq!(json["requests_per_hour"], 100); // Default
    }
}
