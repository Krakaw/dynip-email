# Email Retention Functionality

## Overview

The email retention functionality automatically deletes old emails based on a configurable time period. This helps manage database size and comply with data retention policies.

## Configuration

### Environment Variable

Set the `EMAIL_RETENTION_HOURS` environment variable to enable automatic email deletion:

```bash
# Delete emails older than 24 hours (1 day)
EMAIL_RETENTION_HOURS=24

# Delete emails older than 72 hours (3 days)
EMAIL_RETENTION_HOURS=72

# Delete emails older than 168 hours (1 week)
EMAIL_RETENTION_HOURS=168
```

If `EMAIL_RETENTION_HOURS` is not set or is invalid, email retention is **disabled** and emails are kept indefinitely.

## How It Works

1. **Configuration Loading**: On startup, the application reads the `EMAIL_RETENTION_HOURS` environment variable
2. **Cleanup Task**: If configured, a background task starts that runs every hour
3. **Email Deletion**: The task deletes all emails with timestamps older than the configured retention period
4. **Logging**: The application logs:
   - Startup status (enabled/disabled)
   - Number of emails deleted during each cleanup run
   - Any errors that occur during cleanup

## Implementation Details

### Files Modified

- **src/config.rs**: 
  - Added `email_retention_hours: Option<i64>` field to `Config` struct
  - Loads value from `EMAIL_RETENTION_HOURS` environment variable

- **src/main.rs**: 
  - Spawns periodic cleanup task if retention is enabled
  - Runs cleanup every hour using `tokio::time::interval`
  - Logs startup configuration and cleanup results

- **src/storage/sqlite.rs**: 
  - Implements `delete_old_emails()` method
  - Uses SQL query to delete emails older than cutoff timestamp
  - Returns count of deleted emails

### Database Query

The cleanup uses the following SQL query:

```sql
DELETE FROM emails
WHERE timestamp < ?
```

An index on the `timestamp` column (`idx_timestamp`) ensures efficient cleanup queries.

## Example Usage

### Enable Retention (24 hours)

1. Set the environment variable:
   ```bash
   export EMAIL_RETENTION_HOURS=24
   ```

2. Start the application:
   ```bash
   cargo run
   ```

3. You'll see in the logs:
   ```
   📅 Email retention enabled: emails older than 24 hours will be deleted
   ```

4. Every hour, if old emails are found:
   ```
   🗑️  Email retention cleanup: deleted 5 old email(s)
   ```

### Disable Retention

1. Unset or don't set the environment variable:
   ```bash
   unset EMAIL_RETENTION_HOURS
   ```

2. Start the application:
   ```bash
   cargo run
   ```

3. You'll see in the logs:
   ```
   📅 Email retention disabled: emails will be kept indefinitely
   ```

## Testing

### Manual Testing

1. **Create test emails**: Send several emails to your instance
2. **Manually update timestamps**: Use SQL to set some emails to old timestamps
   ```sql
   UPDATE emails 
   SET timestamp = datetime('now', '-48 hours') 
   WHERE id = 'some-email-id';
   ```
3. **Wait for cleanup**: Wait for the next hourly cleanup cycle or restart the app
4. **Verify deletion**: Check that old emails were removed

### Automated Testing

You can test email retention by:

1. **Set a short retention period**:
   ```bash
   EMAIL_RETENTION_HOURS=1 cargo run
   ```

2. **Send test emails and wait**:
   ```bash
   # Send test emails
   python3 scripts/test_email.py
   
   # Wait for retention cleanup (runs every hour)
   # Or manually update timestamps in database for immediate testing
   ```

3. **Verify cleanup in logs**:
   Look for messages like "🗑️ Email retention cleanup: deleted X old email(s)"

## Monitoring

### Log Messages

| Message | Meaning |
|---------|---------|
| `📅 Email retention enabled: emails older than X hours will be deleted` | Retention is active |
| `📅 Email retention disabled: emails will be kept indefinitely` | Retention is disabled |
| `🗑️ Email retention cleanup: deleted X old email(s)` | Cleanup completed successfully |
| `❌ Email retention cleanup failed: <error>` | Cleanup encountered an error |

### Best Practices

1. **Set appropriate retention period**: Consider your use case
   - Temporary email service: 24-48 hours
   - Testing/development: 1-7 days
   - Production with compliance: Based on requirements

2. **Monitor disk usage**: Even with retention enabled, monitor database growth

3. **Database maintenance**: Periodically run `VACUUM` on SQLite to reclaim space:
   ```bash
   sqlite3 emails.db "VACUUM;"
   ```

## Troubleshooting

### Emails not being deleted

1. Check that `EMAIL_RETENTION_HOURS` is set correctly
2. Verify the application logs show retention is enabled
3. Ensure emails are actually older than the retention period
4. Check for error messages in the logs

### Performance issues

1. Verify the `idx_timestamp` index exists:
   ```sql
   SELECT name FROM sqlite_master 
   WHERE type='index' AND name='idx_timestamp';
   ```

2. Check cleanup frequency (hourly by default)
3. Consider adjusting retention period if too many emails are being deleted at once

## WebSocket Deletion Notifications

The email retention system includes real-time WebSocket notifications when emails are deleted due to retention policies. This ensures the frontend stays synchronized with the backend when emails are automatically removed.

### How It Works

When the retention cleanup task runs (every hour), it:
1. Identifies emails older than the configured retention period
2. Deletes them from the database
3. Broadcasts deletion events via WebSocket to all connected clients

### WebSocket Message Types

The WebSocket supports three message types:

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

### Frontend Handling

The frontend JavaScript automatically:
- Removes deleted emails from the local email list
- Updates the email count
- Clears the detail view if the currently selected email was deleted
- Shows a notification to the user

### Testing Deletion Notifications

You can test deletion notifications by:

1. **Set up retention**:
   ```bash
   EMAIL_RETENTION_HOURS=1 cargo run
   ```

2. **Send test emails**:
   ```bash
   python3 scripts/test_email.py
   ```

3. **Monitor WebSocket**:
   Open the web interface and watch for deletion notifications in the browser console or network tab.

### User Experience

When an email is deleted:
1. **Email List**: The email disappears from the list immediately
2. **Email Count**: The count updates to reflect the new total
3. **Detail View**: If the deleted email was selected, the detail view clears
4. **Notification**: A browser notification appears: "🗑️ Email deleted"

## Future Enhancements

Potential improvements:

- [ ] Configurable cleanup interval (currently fixed at 1 hour)
- [ ] Manual cleanup API endpoint
- [ ] Per-address retention policies
- [ ] Retention metrics/statistics API
- [ ] Soft delete with recovery period

