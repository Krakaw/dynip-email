use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

use crate::auth::AuthenticatedUser;
use crate::rate_limit::RateLimit;
use crate::storage::StorageBackend;

fn require_admin(user: &AuthenticatedUser) -> Result<(), (StatusCode, String)> {
    if user.is_admin {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "Admin access required".to_string()))
    }
}

#[derive(Debug, Deserialize)]
pub struct PageParams {
    limit: Option<i64>,
    offset: Option<i64>,
}

/// List inboxes that have received email, one page at a time
pub async fn list_inboxes(
    Query(page): Query<PageParams>,
    State(storage): State<Arc<dyn StorageBackend>>,
    user: AuthenticatedUser,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&user)?;
    let limit = page.limit.unwrap_or(100).clamp(1, 500);
    let offset = page.offset.unwrap_or(0).max(0);
    let (inboxes, total) = storage
        .list_inboxes(limit, offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "inboxes": inboxes, "total": total })))
}

/// Get every email for an inbox, bypassing mailbox passwords
pub async fn get_inbox_emails(
    Path(address): Path<String>,
    State(storage): State<Arc<dyn StorageBackend>>,
    user: AuthenticatedUser,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&user)?;
    info!(
        "Admin {} read mailbox {} (password bypass)",
        user.email, address
    );
    let emails = storage
        .get_emails_for_address(&address)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "emails": emails })))
}

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
    user: AuthenticatedUser,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&user)?;
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
    user: AuthenticatedUser,
    Json(request): Json<SetRateLimitRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&user)?;
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
    user: AuthenticatedUser,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&user)?;
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
    user: AuthenticatedUser,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&user)?;
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

    fn admin() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: "id".into(),
            email: "admin@example.com".into(),
            is_admin: true,
        }
    }

    #[tokio::test]
    async fn test_rate_limit_routes_require_admin() {
        let storage = create_test_storage().await;
        let non_admin = AuthenticatedUser {
            is_admin: false,
            ..admin()
        };
        let result = get_rate_limit(Path("a@example.com".into()), State(storage), non_admin).await;
        assert_eq!(result.unwrap_err().0, StatusCode::FORBIDDEN);
    }

    async fn create_test_storage() -> Arc<dyn StorageBackend> {
        Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap())
    }

    #[tokio::test]
    async fn test_get_rate_limit_default() {
        let storage = create_test_storage().await;
        let address = "test@example.com".to_string();

        let result = get_rate_limit(Path(address), State(storage), admin()).await;
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

        let set_result = set_rate_limit(
            Path(address.clone()),
            State(storage.clone()),
            admin(),
            Json(request),
        )
        .await;
        assert!(set_result.is_ok());

        let get_result = get_rate_limit(Path(address), State(storage), admin()).await;
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

        let result = set_rate_limit(
            Path(address.clone()),
            State(storage.clone()),
            admin(),
            Json(request),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::BAD_REQUEST);

        // Test hourly > daily
        let request = SetRateLimitRequest {
            requests_per_hour: 1000,
            requests_per_day: 500,
        };

        let result = set_rate_limit(Path(address), State(storage), admin(), Json(request)).await;
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

        set_rate_limit(
            Path(address.clone()),
            State(storage.clone()),
            admin(),
            Json(request),
        )
        .await
        .unwrap();

        // Delete it
        let delete_result =
            delete_rate_limit(Path(address.clone()), State(storage.clone()), admin()).await;
        assert!(delete_result.is_ok());

        // Verify it's gone (returns default)
        let get_result = get_rate_limit(Path(address), State(storage), admin()).await;
        let json = get_result.unwrap().0;
        assert_eq!(json["requests_per_hour"], 100); // Default
    }

    #[tokio::test]
    async fn test_admin_inboxes_require_admin() {
        use crate::storage::models::Email;

        let storage = create_test_storage().await;
        storage
            .store_email(Email::new(
                "box@example.com".into(),
                "s@x.com".into(),
                "hi".into(),
                "body".into(),
                None,
                vec![],
            ))
            .await
            .unwrap();

        let as_user = |is_admin| AuthenticatedUser {
            user_id: "id".into(),
            email: "u@example.com".into(),
            is_admin,
        };

        let first_page = || {
            Query(PageParams {
                limit: None,
                offset: None,
            })
        };

        let denied = list_inboxes(first_page(), State(storage.clone()), as_user(false)).await;
        assert_eq!(denied.unwrap_err().0, StatusCode::FORBIDDEN);
        let denied = get_inbox_emails(
            Path("box@example.com".into()),
            State(storage.clone()),
            as_user(false),
        )
        .await;
        assert_eq!(denied.unwrap_err().0, StatusCode::FORBIDDEN);

        let inboxes = list_inboxes(first_page(), State(storage.clone()), as_user(true))
            .await
            .unwrap()
            .0;
        assert_eq!(inboxes["inboxes"][0]["address"], "box@example.com");
        assert_eq!(inboxes["inboxes"][0]["email_count"], 1);
        assert_eq!(inboxes["total"], 1);

        let emails = get_inbox_emails(
            Path("box@example.com".into()),
            State(storage),
            as_user(true),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(emails["emails"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_list_inboxes_pages() {
        use crate::storage::models::Email;

        let storage = create_test_storage().await;
        for i in 0..5 {
            let mut email = Email::new(
                format!("box{}@example.com", i),
                "s@x.com".into(),
                "hi".into(),
                "body".into(),
                None,
                vec![],
            );
            email.timestamp = chrono::Utc::now() - chrono::Duration::minutes(i);
            storage.store_email(email).await.unwrap();
        }

        let (page, total) = storage.list_inboxes(2, 2).await.unwrap();
        assert_eq!(total, 5);
        let addresses: Vec<_> = page.iter().map(|i| i.address.as_str()).collect();
        assert_eq!(addresses, ["box2@example.com", "box3@example.com"]);

        let (past_end, total) = storage.list_inboxes(2, 10).await.unwrap();
        assert!(past_end.is_empty());
        assert_eq!(total, 5);
    }

    #[test]
    fn test_is_admin_matches_config_case_insensitively() {
        let config = crate::auth::AuthConfig {
            enabled: true,
            jwt_secret: "s".into(),
            jwt_expiry_hours: 1,
            auth_domains: None,
            admin_emails: vec!["boss@example.com".into()],
            outbound_enabled: false,
        };
        assert!(config.is_admin("Boss@Example.com"));
        assert!(!config.is_admin("other@example.com"));
        let disabled = crate::auth::AuthConfig {
            enabled: false,
            ..config
        };
        assert!(!disabled.is_admin("boss@example.com"));
    }
}
