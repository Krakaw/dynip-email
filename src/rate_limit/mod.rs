use anyhow::Result;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

use crate::storage::StorageBackend;

/// Rate limit configuration per user/mailbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub mailbox_address: String,
    pub requests_per_hour: u32,
    pub requests_per_day: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RateLimit {
    /// Create a new rate limit with default values
    pub fn new(mailbox_address: String) -> Self {
        let now = Utc::now();
        Self {
            mailbox_address,
            requests_per_hour: 100, // Default: 100 requests per hour
            requests_per_day: 1000, // Default: 1000 requests per day
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a custom rate limit
    pub fn with_limits(
        mailbox_address: String,
        requests_per_hour: u32,
        requests_per_day: u32,
    ) -> Self {
        let now = Utc::now();
        Self {
            mailbox_address,
            requests_per_hour,
            requests_per_day,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Rate limit request tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRequest {
    pub mailbox_address: String,
    pub timestamp: DateTime<Utc>,
}

impl RateLimitRequest {
    pub fn new(mailbox_address: String) -> Self {
        Self {
            mailbox_address,
            timestamp: Utc::now(),
        }
    }
}

/// Rate limit check result
#[derive(Debug)]
pub struct RateLimitCheck {
    pub allowed: bool,
    pub hourly_count: u32,
    pub hourly_limit: u32,
    pub daily_count: u32,
    pub daily_limit: u32,
    pub retry_after: Option<u64>,
}

/// Check if a request should be allowed based on rate limits
pub async fn check_rate_limit(
    storage: &Arc<dyn StorageBackend>,
    mailbox_address: &str,
) -> Result<RateLimitCheck> {
    // Get or create rate limit for this mailbox
    let rate_limit = match storage.get_rate_limit(mailbox_address).await? {
        Some(limit) => limit,
        None => {
            // Create default rate limit
            let limit = RateLimit::new(mailbox_address.to_string());
            storage.create_rate_limit(limit.clone()).await?;
            limit
        }
    };

    // Get request counts for the last hour and day
    let now = Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);
    let one_day_ago = now - chrono::Duration::days(1);

    let hourly_count = storage
        .count_requests_since(mailbox_address, one_hour_ago)
        .await?;
    let daily_count = storage
        .count_requests_since(mailbox_address, one_day_ago)
        .await?;

    debug!(
        "Rate limit check for {}: {}/{} hourly, {}/{} daily",
        mailbox_address,
        hourly_count,
        rate_limit.requests_per_hour,
        daily_count,
        rate_limit.requests_per_day
    );

    // Check if limits are exceeded
    let hourly_exceeded = hourly_count >= rate_limit.requests_per_hour;
    let daily_exceeded = daily_count >= rate_limit.requests_per_day;

    if hourly_exceeded || daily_exceeded {
        // Calculate retry-after in seconds
        let retry_after = if hourly_exceeded {
            // If hourly limit exceeded, retry after the oldest request in the hour window expires
            let oldest_request_time = storage
                .get_oldest_request_since(mailbox_address, one_hour_ago)
                .await?;
            if let Some(oldest) = oldest_request_time {
                let retry_time = oldest + chrono::Duration::hours(1);
                let seconds_until_retry = (retry_time - now).num_seconds();
                Some(seconds_until_retry.max(0) as u64)
            } else {
                Some(3600) // Default to 1 hour
            }
        } else {
            // Daily limit exceeded
            let oldest_request_time = storage
                .get_oldest_request_since(mailbox_address, one_day_ago)
                .await?;
            if let Some(oldest) = oldest_request_time {
                let retry_time = oldest + chrono::Duration::days(1);
                let seconds_until_retry = (retry_time - now).num_seconds();
                Some(seconds_until_retry.max(0) as u64)
            } else {
                Some(86400) // Default to 24 hours
            }
        };

        Ok(RateLimitCheck {
            allowed: false,
            hourly_count,
            hourly_limit: rate_limit.requests_per_hour,
            daily_count,
            daily_limit: rate_limit.requests_per_day,
            retry_after,
        })
    } else {
        Ok(RateLimitCheck {
            allowed: true,
            hourly_count,
            hourly_limit: rate_limit.requests_per_hour,
            daily_count,
            daily_limit: rate_limit.requests_per_day,
            retry_after: None,
        })
    }
}

/// Record a request for rate limiting
pub async fn record_request(
    storage: &Arc<dyn StorageBackend>,
    mailbox_address: &str,
) -> Result<()> {
    let request = RateLimitRequest::new(mailbox_address.to_string());
    storage.record_rate_limit_request(request).await
}

/// Middleware to enforce rate limits on API requests
pub async fn rate_limit_middleware(
    State(storage): State<Arc<dyn StorageBackend>>,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // Extract mailbox address from request path
    // For now, we'll apply rate limiting to all API endpoints
    // You can customize this logic to extract the mailbox from specific routes

    // Skip rate limiting for auth routes and status endpoints
    let path = request.uri().path();
    if path.starts_with("/api/auth/") || path == "/api/mailbox" {
        return Ok(next.run(request).await);
    }

    // Extract mailbox from path (e.g., /api/emails/:address or /api/mailbox/:address)
    let mailbox_address = extract_mailbox_from_path(path);

    if let Some(address) = mailbox_address {
        // Check rate limit
        match check_rate_limit(&storage, &address).await {
            Ok(check) => {
                if !check.allowed {
                    warn!(
                        "Rate limit exceeded for {}: {}/{} hourly, {}/{} daily",
                        address,
                        check.hourly_count,
                        check.hourly_limit,
                        check.daily_count,
                        check.daily_limit
                    );

                    let retry_after = check.retry_after.unwrap_or(3600);
                    let response = serde_json::json!({
                        "error": "Rate limit exceeded",
                        "hourly_count": check.hourly_count,
                        "hourly_limit": check.hourly_limit,
                        "daily_count": check.daily_count,
                        "daily_limit": check.daily_limit,
                        "retry_after": retry_after
                    });

                    return Err((
                        StatusCode::TOO_MANY_REQUESTS,
                        format!(
                            "{}\nRetry-After: {}",
                            serde_json::to_string(&response).unwrap_or_default(),
                            retry_after
                        ),
                    ));
                }

                // Record the request
                if let Err(e) = record_request(&storage, &address).await {
                    warn!("Failed to record rate limit request: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to check rate limit: {}", e);
                // Continue processing the request even if rate limit check fails
            }
        }
    }

    Ok(next.run(request).await)
}

/// Extract mailbox address from request path
fn extract_mailbox_from_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();

    // Handle different route patterns:
    // /api/emails/:address
    // /api/mailbox/:address/...
    // /api/webhooks/:address
    if parts.len() >= 4 {
        match (parts.get(2), parts.get(3)) {
            (Some(&"emails"), Some(address)) if !address.is_empty() => Some(address.to_string()),
            (Some(&"mailbox"), Some(address)) if !address.is_empty() => Some(address.to_string()),
            (Some(&"webhooks"), Some(address)) if !address.is_empty() => Some(address.to_string()),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mailbox_from_path() {
        assert_eq!(
            extract_mailbox_from_path("/api/emails/test@example.com"),
            Some("test@example.com".to_string())
        );
        assert_eq!(
            extract_mailbox_from_path("/api/mailbox/user/status"),
            Some("user".to_string())
        );
        assert_eq!(
            extract_mailbox_from_path("/api/webhooks/user"),
            Some("user".to_string())
        );
        assert_eq!(extract_mailbox_from_path("/api/auth/login"), None);
        assert_eq!(extract_mailbox_from_path("/api/email/123"), None);
    }

    #[test]
    fn test_rate_limit_creation() {
        let limit = RateLimit::new("test@example.com".to_string());
        assert_eq!(limit.mailbox_address, "test@example.com");
        assert_eq!(limit.requests_per_hour, 100);
        assert_eq!(limit.requests_per_day, 1000);
    }

    #[test]
    fn test_rate_limit_custom() {
        let limit = RateLimit::with_limits("test@example.com".to_string(), 50, 500);
        assert_eq!(limit.mailbox_address, "test@example.com");
        assert_eq!(limit.requests_per_hour, 50);
        assert_eq!(limit.requests_per_day, 500);
    }
}
