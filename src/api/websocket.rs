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

