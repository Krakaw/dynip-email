use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::storage::models::Email;
use serde::{Deserialize, Serialize};

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// New email received
    Email {
        id: String,
        to: String,
        from: String,
        subject: String,
        body: String,
        timestamp: String,
        raw: Option<String>,
        attachments: Vec<crate::storage::models::Attachment>,
    },
    /// Email deleted
    EmailDeleted { id: String, address: String },
    /// Connection established
    Connected { address: String },
}

impl From<Email> for WsMessage {
    fn from(email: Email) -> Self {
        WsMessage::Email {
            id: email.id,
            to: email.to,
            from: email.from,
            subject: email.subject,
            body: email.body,
            timestamp: email.timestamp.to_rfc3339(),
            raw: email.raw,
            attachments: email.attachments,
        }
    }
}

/// WebSocket connection state
#[derive(Clone)]
pub struct WsState {
    pub email_receiver: broadcast::Sender<Email>,
    pub deletion_sender: broadcast::Sender<(String, String)>, // (email_id, address)
    pub domain_name: String,
}

impl WsState {
    /// Normalize an email address by appending domain if not present
    fn normalize_address(&self, input: &str) -> String {
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

/// Handle WebSocket upgrade for a specific email address
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(address): Path<String>,
    State(state): State<WsState>,
) -> Response {
    // Normalize the address (append domain if not present)
    let normalized_address = state.normalize_address(&address);
    info!("WebSocket connection requested for address: {} (normalized: {})", address, normalized_address);
    ws.on_upgrade(move |socket| handle_socket(socket, normalized_address, state))
}

/// Handle individual WebSocket connections
async fn handle_socket(socket: WebSocket, address: String, state: WsState) {
    let (mut sender, mut receiver) = socket.split();
    let mut email_rx = state.email_receiver.subscribe();
    let mut deletion_rx = state.deletion_sender.subscribe();
    
    let address_clone = address.clone();
    info!("WebSocket connected for address: {}", address);
    
    // Send initial connection message
    let connected_msg = WsMessage::Connected { address: address.clone() };
    if let Err(e) = sender
        .send(Message::Text(serde_json::to_string(&connected_msg).unwrap()))
        .await
    {
        error!("Failed to send connection message: {}", e);
        return;
    }
    
    // Spawn a task to handle incoming messages from the client (mostly just pings)
    let address_for_send = address.clone();
    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Handle new emails
                email_result = email_rx.recv() => {
                    if let Ok(email) = email_result {
                        // Only send emails that match this address
                        if email.to == address_for_send {
                            let msg = WsMessage::from(email);
                            let json = match serde_json::to_string(&msg) {
                                Ok(json) => json,
                                Err(e) => {
                                    error!("Failed to serialize email: {}", e);
                                    continue;
                                }
                            };
                            
                            if sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                // Handle email deletions
                deletion_result = deletion_rx.recv() => {
                    if let Ok((email_id, deleted_address)) = deletion_result {
                        info!("📨 Received deletion event for email {} to address {}", email_id, deleted_address);
                        // Only send deletions for this address
                        if deleted_address == address_for_send {
                            let msg = WsMessage::EmailDeleted { 
                                id: email_id.clone(), 
                                address: deleted_address.clone() 
                            };
                            let json = match serde_json::to_string(&msg) {
                                Ok(json) => {
                                    info!("📤 Sending deletion notification: {}", json);
                                    json
                                },
                                Err(e) => {
                                    error!("Failed to serialize deletion: {}", e);
                                    continue;
                                }
                            };
                            
                            if sender.send(Message::Text(json)).await.is_err() {
                                error!("Failed to send deletion notification to WebSocket");
                                break;
                            } else {
                                info!("✅ Deletion notification sent successfully");
                            }
                        } else {
                            info!("⏭️  Skipping deletion notification for different address: {} (current: {})", deleted_address, address_for_send);
                        }
                    }
                }
            }
        }
    });
    
    // Handle incoming messages (ping/pong, close, etc.)
    let address_for_recv = address_clone.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => {
                    info!("WebSocket client disconnected for address: {}", address_for_recv);
                    break;
                }
                Ok(Message::Ping(_)) => {
                    // Respond to ping with pong (handled automatically by axum)
                    info!("Received ping for address: {}", address_for_recv);
                }
                Ok(Message::Pong(_)) => {
                    // Pong received
                }
                Ok(Message::Text(text)) => {
                    info!("Received message for {}: {}", address_for_recv, text);
                }
                Err(e) => {
                    warn!("WebSocket error for address {}: {}", address_for_recv, e);
                    break;
                }
                _ => {}
            }
        }
    });
    
    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
    
    info!("WebSocket closed for address: {}", address_clone);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::Email;
    use serde_json::json;
    use tokio::sync::broadcast;

    fn create_test_ws_state() -> WsState {
        let (email_tx, _) = broadcast::channel::<Email>(100);
        let (deletion_tx, _) = broadcast::channel::<(String, String)>(100);
        
        WsState {
            email_receiver: email_tx,
            deletion_sender: deletion_tx,
            domain_name: "test.local".to_string(),
        }
    }

    #[test]
    fn test_ws_message_from_email() {
        let email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        
        let ws_message = WsMessage::from(email.clone());
        
        match ws_message {
            WsMessage::Email {
                id,
                to,
                from,
                subject,
                body,
                timestamp,
                raw,
                attachments,
            } => {
                assert_eq!(id, email.id);
                assert_eq!(to, email.to);
                assert_eq!(from, email.from);
                assert_eq!(subject, email.subject);
                assert_eq!(body, email.body);
                assert_eq!(raw, email.raw);
                assert_eq!(attachments.len(), email.attachments.len());
                assert!(!timestamp.is_empty());
            }
            _ => panic!("Expected Email message type"),
        }
    }

    #[test]
    fn test_ws_message_serialization() {
        let email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        
        let ws_message = WsMessage::from(email);
        let json = serde_json::to_string(&ws_message).unwrap();
        
        assert!(json.contains("Test Subject"));
        assert!(json.contains("test@test.local"));
        assert!(json.contains("sender@example.com"));
    }

    #[test]
    fn test_ws_message_deserialization() {
        let json = json!({
            "type": "Email",
            "id": "test-id",
            "to": "test@test.local",
            "from": "sender@example.com",
            "subject": "Test Subject",
            "body": "Test body",
            "timestamp": "2024-01-01T00:00:00Z",
            "raw": null,
            "attachments": []
        });
        
        let ws_message: WsMessage = serde_json::from_value(json).unwrap();
        
        match ws_message {
            WsMessage::Email {
                id,
                to,
                from,
                subject,
                body,
                raw,
                attachments,
                ..
            } => {
                assert_eq!(id, "test-id");
                assert_eq!(to, "test@test.local");
                assert_eq!(from, "sender@example.com");
                assert_eq!(subject, "Test Subject");
                assert_eq!(body, "Test body");
                assert_eq!(raw, None);
                assert!(attachments.is_empty());
            }
            _ => panic!("Expected Email message type"),
        }
    }

    #[test]
    fn test_ws_message_email_deleted() {
        let json = json!({
            "type": "EmailDeleted",
            "id": "test-id",
            "address": "test@test.local"
        });
        
        let ws_message: WsMessage = serde_json::from_value(json).unwrap();
        
        match ws_message {
            WsMessage::EmailDeleted { id, address } => {
                assert_eq!(id, "test-id");
                assert_eq!(address, "test@test.local");
            }
            _ => panic!("Expected EmailDeleted message type"),
        }
    }

    #[test]
    fn test_ws_message_connected() {
        let json = json!({
            "type": "Connected",
            "address": "test@test.local"
        });
        
        let ws_message: WsMessage = serde_json::from_value(json).unwrap();
        
        match ws_message {
            WsMessage::Connected { address } => {
                assert_eq!(address, "test@test.local");
            }
            _ => panic!("Expected Connected message type"),
        }
    }

    #[test]
    fn test_ws_state_normalize_address() {
        let state = create_test_ws_state();
        
        // Test normalization of address without @
        assert_eq!(state.normalize_address("user"), "user@test.local");
        
        // Test address with @ should remain unchanged
        assert_eq!(state.normalize_address("user@example.com"), "user@example.com");
        
        // Test address with @ and domain should remain unchanged
        assert_eq!(state.normalize_address("user@test.local"), "user@test.local");
        
        // Test trimming whitespace
        assert_eq!(state.normalize_address("  user  "), "user@test.local");
        
        // Test empty string
        assert_eq!(state.normalize_address(""), "@test.local");
    }

    #[test]
    fn test_ws_message_with_attachments() {
        let mut email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        
        email.attachments.push(crate::storage::models::Attachment {
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 100,
            content: "dGVzdCBjb250ZW50".to_string(),
        });
        
        let ws_message = WsMessage::from(email);
        let json = serde_json::to_string(&ws_message).unwrap();
        
        assert!(json.contains("test.txt"));
        assert!(json.contains("text/plain"));
        assert!(json.contains("dGVzdCBjb250ZW50"));
    }

    #[test]
    fn test_ws_message_with_raw_content() {
        let email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            Some("Raw email content".to_string()),
            vec![],
        );
        
        let ws_message = WsMessage::from(email);
        let json = serde_json::to_string(&ws_message).unwrap();
        
        assert!(json.contains("Raw email content"));
    }

    #[test]
    fn test_ws_message_timestamp_format() {
        let email = Email::new(
            "test@test.local".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        
        let ws_message = WsMessage::from(email);
        let json = serde_json::to_string(&ws_message).unwrap();
        
        // Check that timestamp is in RFC3339 format
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let timestamp = parsed["timestamp"].as_str().unwrap();
        
        // Should be a valid RFC3339 timestamp
        assert!(timestamp.contains("T"));
        assert!(timestamp.contains("Z") || timestamp.contains("+") || timestamp.contains("-"));
    }
}

