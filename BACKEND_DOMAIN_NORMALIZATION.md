# Backend Domain Normalization

## Overview

The server automatically appends the configured domain to email addresses **on the backend** when users submit requests. This ensures security, consistency, and proper handling regardless of the client.

## How It Works

### Domain Normalization Logic

When a user submits an address (e.g., via API or WebSocket), the backend checks:

1. **If the address contains `@`**: Use it as-is (full email address)
2. **If the address doesn't contain `@`**: Append the server's configured domain

```rust
// Backend normalization (happens on server)
fn normalize_address(&self, input: &str) -> String {
    let input = input.trim();
    
    if input.contains('@') {
        input.to_string()  // Full email, use as-is
    } else {
        format!("{}@{}", input, self.domain_name)  // Append domain
    }
}
```

## Where Normalization Happens

### 1. API Endpoint: `/api/emails/:address`

```rust
pub async fn get_emails_for_address(
    Path(address): Path<String>,
    State((storage, config)): State<(Arc<dyn StorageBackend>, AppConfig)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Normalize the address (append domain if not present)
    let normalized_address = config.normalize_address(&address);
    
    // Query database with normalized address
    match storage.get_emails_for_address(&normalized_address).await {
        Ok(emails) => Ok(Json(json!(emails))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch emails: {}", e))),
    }
}
```

### 2. WebSocket Endpoint: `/api/ws/:address`

```rust
pub async fn websocket_handler(
    Path(address): Path<String>,
    State(state): State<WsState>,
) -> Response {
    // Normalize the address (append domain if not present)
    let normalized_address = state.normalize_address(&address);
    
    // Connect WebSocket with normalized address
    ws.on_upgrade(move |socket| handle_socket(socket, normalized_address, state))
}
```

## Examples

### Example 1: Username Only

**Client Request**:
```http
GET /api/emails/test
```

**Backend Processing**:
```
Input: "test"
Domain: "tempmail.local"
Normalized: "test@tempmail.local"
→ Queries database for "test@tempmail.local"
```

### Example 2: Full Email Address

**Client Request**:
```http
GET /api/emails/user@external.com
```

**Backend Processing**:
```
Input: "user@external.com"
Contains '@': Yes
Normalized: "user@external.com" (unchanged)
→ Queries database for "user@external.com"
```

### Example 3: WebSocket Connection

**Client**:
```javascript
const ws = new WebSocket('ws://localhost:3000/api/ws/admin');
```

**Backend Processing**:
```
Input: "admin"
Domain: "tempmail.local"
Normalized: "admin@tempmail.local"
→ Listens for emails to "admin@tempmail.local"
```

## Frontend Behavior

The frontend **does NOT** normalize addresses. It sends whatever the user types:

```javascript
// Load inbox for the specified email address
async function loadInbox() {
    const address = emailAddressInput.value.trim();
    
    // Note: Address normalization happens on the backend
    currentAddress = address;
    
    // Send address as-is to backend
    const response = await fetch(`/api/emails/${address}`);
    // ...
}
```

## Benefits of Backend Normalization

✅ **Security**: Client can't bypass normalization by modifying frontend code  
✅ **Consistency**: All clients (web, mobile, API) get same behavior  
✅ **Single Source of Truth**: Domain configuration lives on server  
✅ **Flexibility**: Easy to change normalization logic without updating clients  
✅ **Debugging**: Server logs show original and normalized addresses  

## Configuration

The domain is set via environment variable:

```env
DOMAIN_NAME=mail.example.com
```

This domain is used for normalization in:
- API endpoint `/api/emails/:address`
- WebSocket endpoint `/api/ws/:address`

## User Experience

### From User's Perspective:

1. **Type "test"** in the web interface
2. **Click "Load Inbox"**
3. **Frontend sends**: `GET /api/emails/test`
4. **Backend normalizes**: `test` → `test@mail.example.com`
5. **Backend queries**: Database for `test@mail.example.com`
6. **Frontend receives**: Emails for `test@mail.example.com`

### With Full Email:

1. **Type "admin@otherdomain.com"** in the web interface
2. **Click "Load Inbox"**
3. **Frontend sends**: `GET /api/emails/admin@otherdomain.com`
4. **Backend checks**: Contains `@` → use as-is
5. **Backend queries**: Database for `admin@otherdomain.com`
6. **Frontend receives**: Emails for `admin@otherdomain.com`

## Frontend Behavior

The frontend has **NO domain logic**. It simply:

1. Accepts user input
2. Sends it to the backend as-is
3. The backend handles all normalization

```javascript
// Frontend just sends whatever the user types
async function loadInbox() {
    const address = emailAddressInput.value.trim();
    // Send to backend without any modification
    const response = await fetch(`/api/emails/${address}`);
}
```

## Implementation Details

### AppConfig Structure

```rust
#[derive(Clone)]
pub struct AppConfig {
    pub domain_name: String,
}

impl AppConfig {
    pub fn normalize_address(&self, input: &str) -> String {
        let input = input.trim();
        if input.contains('@') {
            input.to_string()
        } else {
            format!("{}@{}", input, self.domain_name)
        }
    }
}
```

### WsState Structure

```rust
#[derive(Clone)]
pub struct WsState {
    pub email_receiver: broadcast::Sender<Email>,
    pub domain_name: String,
}

impl WsState {
    fn normalize_address(&self, input: &str) -> String {
        let input = input.trim();
        if input.contains('@') {
            input.to_string()
        } else {
            format!("{}@{}", input, self.domain_name)
        }
    }
}
```

## Testing

### Test API Normalization

```bash
# Start server
DOMAIN_NAME=tempmail.local cargo run

# Test with username
curl http://localhost:3000/api/emails/test
# Backend queries: test@tempmail.local

# Test with full email
curl http://localhost:3000/api/emails/user@external.com
# Backend queries: user@external.com (unchanged)
```

### Test WebSocket Normalization

```javascript
// Connect with username
const ws = new WebSocket('ws://localhost:3000/api/ws/test');
// Backend normalizes to: test@tempmail.local

// Connect with full email
const ws2 = new WebSocket('ws://localhost:3000/api/ws/admin@other.com');
// Backend uses: admin@other.com (unchanged)
```

### Check Server Logs

```bash
# Server logs show both original and normalized addresses:
WebSocket connection requested for address: test (normalized: test@tempmail.local)
WebSocket connection requested for address: admin@other.com (normalized: admin@other.com)
```

## Summary

| Client Input | Domain | Backend Normalizes To |
|-------------|--------|----------------------|
| `test` | `mail.example.com` | `test@mail.example.com` |
| `admin` | `tempmail.local` | `admin@tempmail.local` |
| `user@external.com` | `mail.example.com` | `user@external.com` |
| `hello@world.net` | `tempmail.local` | `hello@world.net` |

**Key Point**: Normalization happens **server-side** for security and consistency. The frontend simply sends whatever the user types, and the backend handles the rest.

