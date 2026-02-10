# Mailbox Password Protection

This feature implements a "first-claim" password protection model for mailboxes, allowing users to lock their mailboxes with a password.

## Overview

- **First-claim model**: The first person to access a mailbox can optionally set a password to lock it
- **Once locked, always locked**: A mailbox with a password cannot be unlocked or have its password changed
- **Password required for all operations**: Once locked, all operations (viewing emails, managing webhooks) require the correct password

## API Endpoints

### Check Mailbox Status

Check if a mailbox is locked (password-protected):

```bash
GET /api/mailbox/:address/status
```

**Response:**
```json
{
  "address": "user@example.com",
  "is_locked": false
}
```

### Claim a Mailbox

Set a password on an unlocked mailbox (first-claim):

```bash
POST /api/mailbox/:address/claim
Content-Type: application/json

{
  "password": "your-secure-password"
}
```

**Success Response:**
```json
{
  "message": "Mailbox claimed successfully",
  "address": "user@example.com"
}
```

**Error Response (409 Conflict):**
```json
"Mailbox is already claimed and locked"
```

## Using Password-Protected Mailboxes

Once a mailbox is locked, all operations require the password as a query parameter:

### Get Emails
```bash
GET /api/emails/:address?password=your-password
```

### Get Webhooks
```bash
GET /api/webhooks/:address?password=your-password
```

### Create Webhook
```bash
POST /api/webhooks
Content-Type: application/json

{
  "mailbox_address": "user@example.com",
  "webhook_url": "https://example.com/webhook",
  "events": ["arrival"],
  "password": "your-password"
}
```

## Error Responses

### 401 Unauthorized
Returned when accessing a locked mailbox without providing a password:
```json
"Mailbox is password protected. Please provide password."
```

### Password Verification Failure
Returned when the provided password is incorrect:
```json
"Password verification error: ..."
```

## Security

- **Password hashing**: Passwords are hashed using bcrypt (cost factor: 12)
- **No password recovery**: If you forget your password, there is no way to recover it or reset it
- **Passwords not stored in plaintext**: Only bcrypt hashes are stored in the database
- **Password never returned**: The password hash is never included in API responses

## Database Schema

A new `mailboxes` table is created:

```sql
CREATE TABLE mailboxes (
    address TEXT PRIMARY KEY,
    password_hash TEXT,
    created_at TEXT NOT NULL,
    is_locked BOOLEAN DEFAULT 0
)
```

## Testing

A test script is provided to verify the password protection functionality:

```bash
./test_password_protection.sh
```

This script tests:
1. Checking unlocked mailbox status
2. Accessing unlocked mailbox without password
3. Claiming mailbox with password
4. Verifying mailbox is locked after claim
5. Denying access without password
6. Rejecting wrong password
7. Granting access with correct password
8. Preventing re-claim of locked mailbox

## Migration

This feature is backward-compatible:
- Existing mailboxes without passwords remain accessible without authentication
- Users can claim their mailboxes at any time
- Once claimed, passwords are required going forward

## Example Workflow

```bash
# 1. Check if mailbox is available
curl http://localhost:3000/api/mailbox/myinbox/status

# 2. Claim the mailbox
curl -X POST http://localhost:3000/api/mailbox/myinbox/claim \
  -H "Content-Type: application/json" \
  -d '{"password":"SecurePassword123!"}'

# 3. Access emails with password
curl "http://localhost:3000/api/emails/myinbox?password=SecurePassword123!"

# 4. Create webhook with password
curl -X POST http://localhost:3000/api/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "mailbox_address": "myinbox",
    "webhook_url": "https://myserver.com/webhook",
    "events": ["arrival"],
    "password": "SecurePassword123!"
  }'
```

## Implementation Details

### Password Verification Flow

1. **Check if mailbox is locked**: Query `mailboxes` table for the address
2. **If unlocked**: Allow access without password
3. **If locked**: Verify password is provided
4. **Hash comparison**: Use bcrypt to compare provided password with stored hash
5. **Grant/deny access**: Based on password match

### Components Modified

- `src/storage/models.rs`: Added `Mailbox` model
- `src/storage/mod.rs`: Added mailbox-related methods to `StorageBackend` trait
- `src/storage/sqlite.rs`: Implemented mailbox methods in SQLite backend
- `src/api/handlers.rs`: Added password verification and claim endpoints
- `src/api/mod.rs`: Registered new API routes
- `Cargo.toml`: Added `bcrypt` dependency

## Security Considerations

⚠️ **Important Security Notes:**

1. **HTTPS Recommended**: Always use HTTPS in production to prevent password interception
2. **Strong Passwords**: Encourage users to use strong, unique passwords
3. **No Password Reset**: There is no password recovery mechanism - choose passwords carefully
4. **Rate Limiting**: Consider implementing rate limiting to prevent brute-force attacks
5. **Audit Logging**: Consider logging authentication attempts for security monitoring

## Future Enhancements

Potential improvements for future versions:

- [ ] Rate limiting on password verification attempts
- [ ] Password strength requirements
- [ ] Session tokens to avoid sending password with every request
- [ ] Multi-factor authentication support
- [ ] Password change functionality (requires current password)
- [ ] Admin override for locked mailboxes
- [ ] Audit logging for authentication events
