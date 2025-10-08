# WebSocket Deletion Notifications

## Overview

The email retention system now includes real-time WebSocket notifications when emails are deleted due to retention policies. This ensures the frontend stays synchronized with the backend when emails are automatically removed.

## How It Works

### 1. Email Retention Cleanup
When the retention cleanup task runs (every hour), it:
1. Identifies emails older than the configured retention period
2. Deletes them from the database
3. Broadcasts deletion events via WebSocket to all connected clients

### 2. WebSocket Message Types
The WebSocket now supports three message types:

```typescript
// New email received
{
  "type": "Email",
  "Email": {
    "id": "uuid",
    "to": "user@example.com",
    "from": "sender@example.com",
    "subject": "Test Email",
    "body": "Email content",
    "timestamp": "2024-01-01T00:00:00Z",
    "attachments": []
  }
}

// Email deleted
{
  "type": "EmailDeleted",
  "id": "uuid",
  "address": "user@example.com"
}

// Connection established
{
  "type": "Connected",
  "address": "user@example.com"
}
```

### 3. Frontend Handling
The frontend JavaScript automatically:
- Removes deleted emails from the local email list
- Updates the email count
- Clears the detail view if the currently selected email was deleted
- Shows a notification to the user

## Implementation Details

### Backend Changes

#### WebSocket Message Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// New email received
    Email(Email),
    /// Email deleted
    EmailDeleted { id: String, address: String },
    /// Connection established
    Connected { address: String },
}
```

#### Enhanced Storage Method
```rust
/// Delete old emails and return details of deleted emails
async fn delete_old_emails_with_details(&self, hours: i64) -> Result<Vec<(String, String)>>;
```

This method:
1. Queries for emails to be deleted (returns ID and address)
2. Deletes the emails from the database
3. Returns the list of deleted email IDs and addresses

#### Retention Cleanup Integration
```rust
// Send deletion notifications for each deleted email
for (email_id, address) in deleted_emails {
    let _ = deletion_tx_clone.send((email_id, address));
}
```

### Frontend Changes

#### WebSocket Message Handler
```javascript
websocket.onmessage = (event) => {
    const data = JSON.parse(event.data);
    
    // Handle new emails
    if (data.type === 'Email' && data.Email) {
        const email = data.Email;
        email.isNew = true;
        emails.unshift(email);
        displayEmails(emails);
        updateEmailCount(emails.length);
        showNotification('📬 New email received!', `From: ${email.from}`);
    }
    
    // Handle email deletions
    if (data.type === 'EmailDeleted') {
        const { id, address } = data;
        
        // Remove from local array
        emails = emails.filter(email => email.id !== id);
        displayEmails(emails);
        updateEmailCount(emails.length);
        
        // Clear detail view if deleted email was selected
        if (selectedEmailId === id) {
            selectedEmailId = null;
            emailDetail.innerHTML = '<div class="empty-state"><p>📭 Select an email to view</p></div>';
        }
        
        showNotification('🗑️ Email deleted', 'An email was removed due to retention policy');
    }
};
```

## Testing

### Manual Testing
1. Set `EMAIL_RETENTION_HOURS=1` in your environment
2. Start the application
3. Send some test emails
4. Wait for the hourly cleanup or manually update email timestamps:
   ```sql
   UPDATE emails SET timestamp = datetime('now', '-2 hours') WHERE to_address = 'test@example.com';
   ```
5. Observe the deletion notifications in the frontend

### Automated Testing
Use the provided test script:
```bash
./scripts/test_deletion_notifications.sh
```

This script:
- Creates a test environment with 1-hour retention
- Starts the application
- Provides a test HTML page for monitoring WebSocket messages
- Includes instructions for manual testing

## User Experience

### Visual Feedback
When an email is deleted:
1. **Email List**: The email disappears from the list immediately
2. **Email Count**: The count updates to reflect the new total
3. **Detail View**: If the deleted email was selected, the detail view clears
4. **Notification**: A browser notification appears: "🗑️ Email deleted"

### Real-time Updates
- No page refresh required
- Instant synchronization across all connected clients
- Seamless user experience during retention cleanup

## Configuration

### Environment Variables
```bash
# Enable email retention (required for deletion notifications)
EMAIL_RETENTION_HOURS=24

# Other configuration
SMTP_PORT=2525
API_PORT=3000
DOMAIN_NAME=tempmail.local
```

### WebSocket Connection
```javascript
const ws = new WebSocket('ws://localhost:3000/api/ws/user@example.com');
```

## Error Handling

### Backend
- Deletion events are sent even if some WebSocket clients are disconnected
- Failed WebSocket sends don't affect the deletion process
- Errors are logged but don't stop the retention cleanup

### Frontend
- Graceful handling of malformed WebSocket messages
- Automatic reconnection on WebSocket errors
- Fallback to manual refresh if needed

## Performance Considerations

### Database Queries
- The `delete_old_emails_with_details` method uses two queries:
  1. SELECT to get email details before deletion
  2. DELETE to remove the emails
- This is necessary to get the email IDs and addresses for notifications

### WebSocket Broadcasting
- Each deleted email generates one WebSocket message per connected client
- Messages are sent asynchronously and don't block the cleanup process
- Failed sends are ignored (no retry mechanism)

### Memory Usage
- Frontend maintains a local copy of emails in memory
- Deleted emails are immediately removed from the local array
- No memory leaks from deleted email references

## Troubleshooting

### Common Issues

1. **Deletions not appearing in frontend**
   - Check WebSocket connection status
   - Verify `EMAIL_RETENTION_HOURS` is set
   - Check browser console for WebSocket errors

2. **Multiple deletion notifications**
   - This is normal if multiple clients are connected
   - Each client receives its own notification

3. **WebSocket connection drops**
   - The frontend will attempt to reconnect
   - Manual page refresh may be needed in some cases

### Debug Information
Enable debug logging:
```bash
RUST_LOG=debug cargo run
```

Check WebSocket messages in browser console:
```javascript
// Add to browser console
websocket.onmessage = (event) => {
    console.log('WebSocket message:', JSON.parse(event.data));
};
```

## Future Enhancements

Potential improvements:
- [ ] Batch deletion notifications for better performance
- [ ] Soft delete with recovery period
- [ ] Deletion confirmation dialogs
- [ ] Email deletion API endpoint
- [ ] Deletion history/audit log
- [ ] Per-address deletion policies

## Security Considerations

- Deletion notifications only sent to clients connected to the specific email address
- No sensitive email content is included in deletion messages
- WebSocket connections are not authenticated (same as email access)
- Deletion events are logged for audit purposes
