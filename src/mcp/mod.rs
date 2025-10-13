use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info};

use crate::storage::{models::{Email, Webhook, WebhookEvent}, StorageBackend};
use crate::webhooks::WebhookTrigger;

/// MCP server implementation for email management
pub struct EmailMcpServer {
    storage: Arc<dyn StorageBackend>,
    webhook_trigger: WebhookTrigger,
}

impl EmailMcpServer {
    /// Create a new MCP server
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        let webhook_trigger = WebhookTrigger::new(storage.clone());
        Self {
            storage,
            webhook_trigger,
        }
    }

    /// Start the MCP server
    pub async fn start(&self, port: u16) -> Result<()> {
        info!("Starting MCP server on port {}", port);
        
        let app = self.create_router();
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        
        info!("🔌 MCP server listening on port {}", port);
        axum::serve(listener, app).await?;
        
        Ok(())
    }

    /// Create the MCP server router
    fn create_router(&self) -> Router {
        let storage = self.storage.clone();
        let webhook_trigger = self.webhook_trigger.clone();
        
        Router::new()
            .route("/", get(Self::handle_root))
            .route("/tools", get(Self::handle_list_tools))
            .route("/tools/:name", post(Self::handle_call_tool))
            .route("/resources", get(Self::handle_list_resources))
            .route("/resources/:id", get(Self::handle_read_resource))
            .with_state((storage, webhook_trigger))
    }

    /// MCP server handlers
    async fn handle_root() -> Json<Value> {
        Json(json!({
            "name": "dynip-email-mcp",
            "version": "1.0.0",
            "description": "Email management MCP server for dynip-email",
            "capabilities": {
                "tools": true,
                "resources": true
            }
        }))
    }

    async fn handle_list_tools() -> Json<Value> {
        Json(json!({
            "tools": [
                {
                    "name": "list_emails",
                    "description": "List emails for a specific mailbox",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "mailbox": {
                                "type": "string",
                                "description": "Mailbox name (without domain)"
                            }
                        },
                        "required": ["mailbox"]
                    }
                },
                {
                    "name": "read_email",
                    "description": "Get a specific email by ID",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "email_id": {
                                "type": "string",
                                "description": "Email ID"
                            }
                        },
                        "required": ["email_id"]
                    }
                },
                {
                    "name": "create_webhook",
                    "description": "Create a new webhook for a mailbox",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "mailbox": {
                                "type": "string",
                                "description": "Mailbox name (without domain)"
                            },
                            "webhook_url": {
                                "type": "string",
                                "description": "Webhook URL"
                            },
                            "events": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "Events to subscribe to"
                            }
                        },
                        "required": ["mailbox", "webhook_url", "events"]
                    }
                },
                {
                    "name": "list_webhooks",
                    "description": "List webhooks for a mailbox",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "mailbox": {
                                "type": "string",
                                "description": "Mailbox name (without domain)"
                            }
                        },
                        "required": ["mailbox"]
                    }
                }
            ]
        }))
    }

    async fn handle_call_tool(
        Path(tool_name): Path<String>,
        State((storage, webhook_trigger)): State<(Arc<dyn StorageBackend>, WebhookTrigger)>,
        Json(payload): Json<Value>,
    ) -> Result<Json<Value>, (StatusCode, String)> {
        match tool_name.as_str() {
            "list_emails" => {
                let mailbox = payload.get("mailbox")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing mailbox parameter".to_string()))?;
                
                match storage.get_emails_for_address(mailbox).await {
                    Ok(emails) => Ok(Json(json!({
                        "emails": emails,
                        "count": emails.len()
                    }))),
                    Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
                }
            }
            "read_email" => {
                let email_id = payload.get("email_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing email_id parameter".to_string()))?;
                
                match storage.get_email_by_id(email_id).await {
                    Ok(Some(email)) => Ok(Json(json!(email))),
                    Ok(None) => Err((StatusCode::NOT_FOUND, "Email not found".to_string())),
                    Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
                }
            }
            "create_webhook" => {
                let mailbox = payload.get("mailbox")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing mailbox parameter".to_string()))?;
                let webhook_url = payload.get("webhook_url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing webhook_url parameter".to_string()))?;
                let events = payload.get("events")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing events parameter".to_string()))?;
                
                let webhook_events: Result<Vec<WebhookEvent>, _> = events
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| WebhookEvent::from_str(s).ok_or_else(|| format!("Invalid event: {}", s)))
                    .collect();
                
                let webhook_events = webhook_events.map_err(|e| (StatusCode::BAD_REQUEST, e))?;
                let webhook = Webhook::new(mailbox.to_string(), webhook_url.to_string(), webhook_events);
                
                match storage.create_webhook(webhook.clone()).await {
                    Ok(_) => Ok(Json(json!(webhook))),
                    Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
                }
            }
            "list_webhooks" => {
                let mailbox = payload.get("mailbox")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing mailbox parameter".to_string()))?;
                
                match storage.get_webhooks_for_mailbox(mailbox).await {
                    Ok(webhooks) => Ok(Json(json!({
                        "webhooks": webhooks,
                        "count": webhooks.len()
                    }))),
                    Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
                }
            }
            _ => Err((StatusCode::NOT_FOUND, "Tool not found".to_string())),
        }
    }

    async fn handle_list_resources() -> Json<Value> {
        Json(json!({
            "resources": [
                {
                    "uri": "email://*",
                    "name": "Email",
                    "description": "Email content resource",
                    "mimeType": "application/json"
                },
                {
                    "uri": "webhook://*",
                    "name": "Webhook",
                    "description": "Webhook configuration resource",
                    "mimeType": "application/json"
                }
            ]
        }))
    }

    async fn handle_read_resource(
        Path(resource_id): Path<String>,
        State((storage, _webhook_trigger)): State<(Arc<dyn StorageBackend>, WebhookTrigger)>,
    ) -> Result<Json<Value>, (StatusCode, String)> {
        if resource_id.starts_with("email://") {
            let email_id = resource_id.strip_prefix("email://").unwrap();
            match storage.get_email_by_id(email_id).await {
                Ok(Some(email)) => Ok(Json(json!(email))),
                Ok(None) => Err((StatusCode::NOT_FOUND, "Email not found".to_string())),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
            }
        } else if resource_id.starts_with("webhook://") {
            let webhook_id = resource_id.strip_prefix("webhook://").unwrap();
            match storage.get_webhook_by_id(webhook_id).await {
                Ok(Some(webhook)) => Ok(Json(json!(webhook))),
                Ok(None) => Err((StatusCode::NOT_FOUND, "Webhook not found".to_string())),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
            }
        } else {
            Err((StatusCode::NOT_FOUND, "Resource not found".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::SqliteBackend;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_mcp_server_creation() {
        let storage = Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap());
        let _server = EmailMcpServer::new(storage);
        
        // Test that server can be created
        assert!(true);
    }

    #[tokio::test]
    async fn test_mcp_server_info() {
        
        let storage = Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap());
        let server = EmailMcpServer::new(storage);
        let app = server.create_router();
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let info: serde_json::Value = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(info["name"], "dynip-email-mcp");
        assert_eq!(info["version"], "1.0.0");
        assert!(info["capabilities"]["tools"].as_bool().unwrap());
        assert!(info["capabilities"]["resources"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_mcp_list_tools() {
        
        let storage = Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap());
        let server = EmailMcpServer::new(storage);
        let app = server.create_router();
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let tools: serde_json::Value = serde_json::from_slice(&body).unwrap();
        
        assert!(tools["tools"].is_array());
        let tools_array = tools["tools"].as_array().unwrap();
        assert!(tools_array.len() >= 4); // list_emails, read_email, create_webhook, list_webhooks
        
        // Check for specific tools
        let tool_names: Vec<&str> = tools_array
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        
        assert!(tool_names.contains(&"list_emails"));
        assert!(tool_names.contains(&"read_email"));
        assert!(tool_names.contains(&"create_webhook"));
        assert!(tool_names.contains(&"list_webhooks"));
    }

    #[tokio::test]
    async fn test_mcp_list_resources() {
        
        let storage = Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap());
        let server = EmailMcpServer::new(storage);
        let app = server.create_router();
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/resources")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let resources: serde_json::Value = serde_json::from_slice(&body).unwrap();
        
        assert!(resources["resources"].is_array());
        let resources_array = resources["resources"].as_array().unwrap();
        assert!(resources_array.len() >= 2); // email://, webhook://
        
        // Check for specific resources
        let resource_uris: Vec<&str> = resources_array
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        
        assert!(resource_uris.contains(&"email://*"));
        assert!(resource_uris.contains(&"webhook://*"));
    }

    #[tokio::test]
    async fn test_mcp_call_tool_list_emails() {
        
        let storage = Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap());
        let server = EmailMcpServer::new(storage);
        let app = server.create_router();
        
        let request_body = json!({
            "mailbox": "test"
        });
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tools/list_emails")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        
        assert!(result["emails"].is_array());
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_mcp_call_tool_invalid_tool() {
        
        let storage = Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap());
        let server = EmailMcpServer::new(storage);
        let app = server.create_router();
        
        let request_body = json!({
            "mailbox": "test"
        });
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tools/invalid_tool")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_mcp_call_tool_missing_parameters() {
        
        let storage = Arc::new(SqliteBackend::new("sqlite::memory:").await.unwrap());
        let server = EmailMcpServer::new(storage);
        let app = server.create_router();
        
        let request_body = json!({});
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tools/list_emails")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
