//! Authentication module for user-based API access control
//!
//! This module provides JWT-based authentication when AUTH_ENABLED is true.
//! When disabled, all API routes are publicly accessible.

use axum::{
    async_trait,
    body::Body,
    extract::{FromRequestParts, State},
    http::{header::AUTHORIZATION, request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::storage::{models::User, StorageBackend};

/// JWT claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// User ID
    pub sub: String,
    /// Email (used as username)
    pub email: String,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
}

/// Auth configuration passed to handlers
#[derive(Clone)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
    /// Optional domain restrictions for registration (e.g., vec!["example.com", "company.com"])
    pub auth_domains: Option<Vec<String>>,
}

/// Request body for registration
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

/// Request body for login
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Generate a JWT token for a user
pub fn generate_token(
    user: &User,
    config: &AuthConfig,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let exp = now + Duration::hours(config.jwt_expiry_hours as i64);

    let claims = Claims {
        sub: user.id.clone(),
        email: user.email.clone(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
}

/// Validate email format
fn is_valid_email(email: &str) -> bool {
    // Basic email validation
    if email.len() < 3 || email.len() > 254 {
        return false;
    }

    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    // Local part validation
    if local.is_empty() || local.len() > 64 {
        return false;
    }

    // Domain validation
    if domain.is_empty() || !domain.contains('.') {
        return false;
    }

    true
}

/// Validate email domain against allowed domains
fn is_allowed_domain(email: &str, allowed_domains: &[String]) -> bool {
    if let Some(at_pos) = email.rfind('@') {
        let email_domain = &email[at_pos + 1..];
        allowed_domains
            .iter()
            .any(|domain| email_domain.eq_ignore_ascii_case(domain))
    } else {
        false
    }
}

/// Verify a JWT token and return claims
pub fn verify_token(
    token: &str,
    config: &AuthConfig,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

/// Register a new user
pub async fn register(
    State((storage, config)): State<(Arc<dyn StorageBackend>, AuthConfig)>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !config.enabled {
        return Err((
            StatusCode::NOT_FOUND,
            "Authentication is not enabled".to_string(),
        ));
    }

    // Validate email format
    if !is_valid_email(&request.email) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid email address format".to_string(),
        ));
    }

    // Validate email domain if restriction is set
    if let Some(ref allowed_domains) = config.auth_domains {
        if !is_allowed_domain(&request.email, allowed_domains) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Registration is not allowed for this email domain".to_string(),
            ));
        }
    }

    // Validate password
    if request.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Check if email already exists
    if storage
        .get_user_by_email(&request.email)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some()
    {
        return Err((StatusCode::CONFLICT, "Email already registered".to_string()));
    }

    // Hash password
    let password_hash = bcrypt::hash(&request.password, bcrypt::DEFAULT_COST).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to hash password: {}", e),
        )
    })?;

    // Create user
    let user = User::new(request.email.clone(), password_hash);
    storage
        .create_user(user.clone())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Generate token
    let token = generate_token(&user, &config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate token: {}", e),
        )
    })?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user.id,
            "email": user.email
        }
    })))
}

/// Login an existing user
pub async fn login(
    State((storage, config)): State<(Arc<dyn StorageBackend>, AuthConfig)>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !config.enabled {
        return Err((
            StatusCode::NOT_FOUND,
            "Authentication is not enabled".to_string(),
        ));
    }

    // Find user by email
    let user = storage
        .get_user_by_email(&request.email)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()))?;

    // Verify password
    let password_valid = bcrypt::verify(&request.password, &user.password_hash).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Password verification error: {}", e),
        )
    })?;

    if !password_valid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()));
    }

    // Generate token
    let token = generate_token(&user, &config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate token: {}", e),
        )
    })?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user.id,
            "email": user.email
        }
    })))
}

/// Get current user info
pub async fn me(
    State((storage, config)): State<(Arc<dyn StorageBackend>, AuthConfig)>,
    claims: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !config.enabled {
        return Err((
            StatusCode::NOT_FOUND,
            "Authentication is not enabled".to_string(),
        ));
    }

    let user = storage
        .get_user_by_id(&claims.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    Ok(Json(json!({
        "id": user.id,
        "email": user.email,
        "created_at": user.created_at
    })))
}

/// Get auth status (whether auth is enabled and if users exist)
pub async fn status(
    State((storage, config)): State<(Arc<dyn StorageBackend>, AuthConfig)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let has_users = if config.enabled {
        storage
            .has_users()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        false
    };

    Ok(Json(json!({
        "auth_enabled": config.enabled,
        "has_users": has_users,
        "registration_open": config.enabled && !has_users,
        "domain_restricted": config.auth_domains.is_some()
    })))
}

/// Authenticated user extracted from JWT
#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: String,
    #[allow(dead_code)]
    pub email: String,
}

/// Extractor for authenticated requests
/// When auth is enabled, this extracts the user from the JWT token.
/// When auth is disabled, this creates a dummy user.
#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get auth config from extensions (set by middleware)
        let auth_config = parts
            .extensions
            .get::<AuthConfig>()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Auth config not found".to_string(),
                )
            })?
            .clone();

        // If auth is disabled, return a dummy user
        if !auth_config.enabled {
            return Ok(AuthenticatedUser {
                user_id: "anonymous".to_string(),
                email: "anonymous".to_string(),
            });
        }

        // Extract Bearer token
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    "Missing authorization header".to_string(),
                )
            })?;

        let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid authorization header format".to_string(),
            )
        })?;

        // Verify token
        let claims = verify_token(token, &auth_config)
            .map_err(|e| (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)))?;

        Ok(AuthenticatedUser {
            user_id: claims.sub,
            email: claims.email,
        })
    }
}

/// Middleware to inject auth config into request extensions
pub async fn auth_config_middleware(
    State(config): State<AuthConfig>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    request.extensions_mut().insert(config);
    next.run(request).await
}

/// Middleware to require authentication when auth is enabled
pub async fn require_auth(
    State(config): State<AuthConfig>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // If auth is disabled, skip authentication
    if !config.enabled {
        return next.run(request).await;
    }

    // Extract and verify token
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            match verify_token(token, &config) {
                Ok(_) => next.run(request).await,
                Err(e) => {
                    (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)).into_response()
                }
            }
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            "Missing or invalid authorization header",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_token() {
        let config = AuthConfig {
            enabled: true,
            jwt_secret: "test-secret-key".to_string(),
            jwt_expiry_hours: 24,
            auth_domains: None,
        };

        let user = User::new("test@example.com".to_string(), "hash".to_string());
        let token = generate_token(&user, &config).unwrap();

        let claims = verify_token(&token, &config).unwrap();
        assert_eq!(claims.sub, user.id);
        assert_eq!(claims.email, user.email);
    }

    #[test]
    fn test_invalid_token() {
        let config = AuthConfig {
            enabled: true,
            jwt_secret: "test-secret-key".to_string(),
            jwt_expiry_hours: 24,
            auth_domains: None,
        };

        let result = verify_token("invalid-token", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_secret() {
        let config1 = AuthConfig {
            enabled: true,
            jwt_secret: "secret1".to_string(),
            jwt_expiry_hours: 24,
            auth_domains: None,
        };

        let config2 = AuthConfig {
            enabled: true,
            jwt_secret: "secret2".to_string(),
            jwt_expiry_hours: 24,
            auth_domains: None,
        };

        let user = User::new("test@example.com".to_string(), "hash".to_string());
        let token = generate_token(&user, &config1).unwrap();

        let result = verify_token(&token, &config2);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_email() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("user.name@domain.co.uk"));
        assert!(is_valid_email("a@b.c"));
    }

    #[test]
    fn test_invalid_email() {
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("no-at-sign"));
        assert!(!is_valid_email("@nodomain"));
        assert!(!is_valid_email("noat@"));
        assert!(!is_valid_email("no@dots"));
    }

    #[test]
    fn test_allowed_domain() {
        let domains = vec!["example.com".to_string(), "company.com".to_string()];
        assert!(is_allowed_domain("user@example.com", &domains));
        assert!(is_allowed_domain("user@EXAMPLE.COM", &domains));
        assert!(is_allowed_domain("user@company.com", &domains));
        assert!(is_allowed_domain("user@COMPANY.COM", &domains));
        assert!(!is_allowed_domain("user@other.com", &domains));
        assert!(!is_allowed_domain("user@example.com.evil.com", &domains));
    }

    // --- Handler integration tests ---

    use axum::{
        body::Body,
        http::{header, Request},
        middleware,
        routing::{get, post},
        Router,
    };
    use tower::util::ServiceExt;

    fn test_auth_config() -> AuthConfig {
        AuthConfig {
            enabled: true,
            jwt_secret: "test-secret-key-for-testing".to_string(),
            jwt_expiry_hours: 24,
            auth_domains: None,
        }
    }

    async fn test_storage() -> Arc<dyn StorageBackend> {
        Arc::new(
            crate::storage::sqlite::SqliteBackend::new("sqlite::memory:")
                .await
                .unwrap(),
        )
    }

    fn auth_app(storage: Arc<dyn StorageBackend>, config: AuthConfig) -> Router {
        Router::new()
            .route("/api/auth/register", post(register))
            .route("/api/auth/login", post(login))
            .route("/api/auth/me", get(me))
            .route("/api/auth/status", get(status))
            .with_state((storage, config.clone()))
            .layer(middleware::from_fn_with_state(
                config,
                auth_config_middleware,
            ))
    }

    async fn register_user(
        app: &Router,
        email: &str,
        password: &str,
    ) -> axum::http::Response<Body> {
        let body = serde_json::json!({
            "email": email,
            "password": password,
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn login_user(app: &Router, email: &str, password: &str) -> axum::http::Response<Body> {
        let body = serde_json::json!({
            "email": email,
            "password": password,
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn body_json(response: axum::http::Response<Body>) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // Registration tests

    #[tokio::test]
    async fn test_register_success() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage, config);

        let response = register_user(&app, "user@example.com", "password123").await;
        assert_eq!(response.status(), StatusCode::OK);

        let json = body_json(response).await;
        assert!(json["token"].is_string());
        assert_eq!(json["user"]["email"], "user@example.com");
        assert!(json["user"]["id"].is_string());
    }

    #[tokio::test]
    async fn test_register_auth_disabled() {
        let storage = test_storage().await;
        let config = AuthConfig {
            enabled: false,
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = register_user(&app, "user@example.com", "password123").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_register_invalid_email() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage, config);

        let response = register_user(&app, "not-an-email", "password123").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_register_short_password() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage, config);

        let response = register_user(&app, "user@example.com", "short").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_register_duplicate_email() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage.clone(), config.clone());

        // First registration succeeds
        let response = register_user(&app, "user@example.com", "password123").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Second registration with same email fails
        let app2 = auth_app(storage, config);
        let response = register_user(&app2, "user@example.com", "password456").await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_register_domain_restriction_allowed() {
        let storage = test_storage().await;
        let config = AuthConfig {
            auth_domains: Some(vec!["allowed.com".to_string()]),
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = register_user(&app, "user@allowed.com", "password123").await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_register_domain_restriction_blocked() {
        let storage = test_storage().await;
        let config = AuthConfig {
            auth_domains: Some(vec!["allowed.com".to_string()]),
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = register_user(&app, "user@blocked.com", "password123").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_register_multiple_domains_allowed() {
        let storage = test_storage().await;
        let config = AuthConfig {
            auth_domains: Some(vec!["allowed.com".to_string(), "company.com".to_string()]),
            ..test_auth_config()
        };
        let app = auth_app(storage.clone(), config.clone());

        // Test first domain
        let response = register_user(&app, "user1@allowed.com", "password123").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Test second domain
        let app2 = auth_app(storage, config);
        let response = register_user(&app2, "user2@company.com", "password456").await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_register_multiple_domains_blocked() {
        let storage = test_storage().await;
        let config = AuthConfig {
            auth_domains: Some(vec!["allowed.com".to_string(), "company.com".to_string()]),
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = register_user(&app, "user@blocked.com", "password123").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // Login tests

    #[tokio::test]
    async fn test_login_success() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage.clone(), config.clone());

        // Register first
        let response = register_user(&app, "user@example.com", "password123").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Login
        let app2 = auth_app(storage, config);
        let response = login_user(&app2, "user@example.com", "password123").await;
        assert_eq!(response.status(), StatusCode::OK);

        let json = body_json(response).await;
        assert!(json["token"].is_string());
        assert_eq!(json["user"]["email"], "user@example.com");
    }

    #[tokio::test]
    async fn test_login_auth_disabled() {
        let storage = test_storage().await;
        let config = AuthConfig {
            enabled: false,
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = login_user(&app, "user@example.com", "password123").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage.clone(), config.clone());

        // Register
        let response = register_user(&app, "user@example.com", "password123").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Login with wrong password
        let app2 = auth_app(storage, config);
        let response = login_user(&app2, "user@example.com", "wrongpassword").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_login_nonexistent_user() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage, config);

        let response = login_user(&app, "nobody@example.com", "password123").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // Status tests

    #[tokio::test]
    async fn test_status_auth_enabled_no_users() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["auth_enabled"], true);
        assert_eq!(json["has_users"], false);
        assert_eq!(json["registration_open"], true);
    }

    #[tokio::test]
    async fn test_status_auth_enabled_with_users() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage.clone(), config.clone());

        // Register a user first
        register_user(&app, "user@example.com", "password123").await;

        let app2 = auth_app(storage, config);
        let response = app2
            .oneshot(
                Request::builder()
                    .uri("/api/auth/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["auth_enabled"], true);
        assert_eq!(json["has_users"], true);
        assert_eq!(json["registration_open"], false);
    }

    #[tokio::test]
    async fn test_status_auth_disabled() {
        let storage = test_storage().await;
        let config = AuthConfig {
            enabled: false,
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["auth_enabled"], false);
        assert_eq!(json["has_users"], false);
        assert_eq!(json["registration_open"], false);
    }

    // Me endpoint tests

    #[tokio::test]
    async fn test_me_with_valid_token() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage.clone(), config.clone());

        // Register and get token
        let response = register_user(&app, "user@example.com", "password123").await;
        let json = body_json(response).await;
        let token = json["token"].as_str().unwrap();

        // Call /me with token
        let app2 = auth_app(storage, config);
        let response = app2
            .oneshot(
                Request::builder()
                    .uri("/api/auth/me")
                    .header(header::AUTHORIZATION, format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["email"], "user@example.com");
        assert!(json["id"].is_string());
    }

    #[tokio::test]
    async fn test_me_without_token() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should fail since no token and auth is enabled
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_me_with_invalid_token() {
        let storage = test_storage().await;
        let config = test_auth_config();
        let app = auth_app(storage, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/me")
                    .header(header::AUTHORIZATION, "Bearer invalid-token-here")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_me_auth_disabled() {
        let storage = test_storage().await;
        let config = AuthConfig {
            enabled: false,
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // Require auth middleware tests

    async fn dummy_handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn test_require_auth_skips_when_disabled() {
        let config = AuthConfig {
            enabled: false,
            ..test_auth_config()
        };
        let app = Router::new()
            .route("/protected", get(dummy_handler))
            .layer(middleware::from_fn_with_state(config, require_auth));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_auth_blocks_without_token() {
        let config = test_auth_config();
        let app = Router::new()
            .route("/protected", get(dummy_handler))
            .layer(middleware::from_fn_with_state(config, require_auth));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_require_auth_passes_with_valid_token() {
        let config = test_auth_config();
        let user = User::new("test@example.com".to_string(), "hash".to_string());
        let token = generate_token(&user, &config).unwrap();

        let app = Router::new()
            .route("/protected", get(dummy_handler))
            .layer(middleware::from_fn_with_state(config, require_auth));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(header::AUTHORIZATION, format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_auth_rejects_invalid_token() {
        let config = test_auth_config();
        let app = Router::new()
            .route("/protected", get(dummy_handler))
            .layer(middleware::from_fn_with_state(config, require_auth));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(header::AUTHORIZATION, "Bearer bad-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // Token claims tests

    #[test]
    fn test_token_claims_contain_correct_fields() {
        let config = test_auth_config();
        let user = User::new("test@example.com".to_string(), "hash".to_string());
        let token = generate_token(&user, &config).unwrap();

        let claims = verify_token(&token, &config).unwrap();
        assert_eq!(claims.sub, user.id);
        assert_eq!(claims.email, "test@example.com");
        assert!(claims.exp > claims.iat);
        assert!(claims.exp - claims.iat == 24 * 3600);
    }

    #[test]
    fn test_token_expiry_hours_configurable() {
        let config = AuthConfig {
            jwt_expiry_hours: 48,
            ..test_auth_config()
        };
        let user = User::new("test@example.com".to_string(), "hash".to_string());
        let token = generate_token(&user, &config).unwrap();

        let claims = verify_token(&token, &config).unwrap();
        assert!(claims.exp - claims.iat == 48 * 3600);
    }

    // Status with domain restriction test

    #[tokio::test]
    async fn test_status_shows_domain_restricted() {
        let storage = test_storage().await;
        let config = AuthConfig {
            auth_domains: Some(vec!["corp.com".to_string(), "company.com".to_string()]),
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["domain_restricted"], true);
    }

    #[tokio::test]
    async fn test_status_no_domain_restriction() {
        let storage = test_storage().await;
        let config = AuthConfig {
            auth_domains: None,
            ..test_auth_config()
        };
        let app = auth_app(storage, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert_eq!(json["domain_restricted"], false);
    }
}
