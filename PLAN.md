# Temporary Mail Server - Project Plan

## Status: ✅ IMPLEMENTED

## Overview
A lightweight temporary mail server that accepts emails to any address, stores them, and provides a web interface to view them with real-time updates.

**Implementation completed!** See [QUICKSTART.md](QUICKSTART.md) to get started.

## Architecture

### Part 1: Email Service (SMTP Server + Storage)

#### SMTP Server
- Accept emails on port 2525 (configurable)
- Use `mailin-embedded` or custom SMTP handler with `tokio`
- Accept all emails to any address (no validation/authentication)
- Parse email content (headers, body, attachments)

#### Storage Layer
- **Storage Backend Trait** - Swappable storage implementation
  ```rust
  trait StorageBackend {
      async fn store_email(&self, email: Email) -> Result<()>;
      async fn get_emails_for_address(&self, address: String) -> Result<Vec<Email>>;
      async fn get_email_by_id(&self, id: String) -> Result<Email>;
  }
  ```

- **Default Implementation**: SQLite via `SqliteBackend`

- **Email Model**:
  - `id`: UUID
  - `to`: Recipient address
  - `from`: Sender address
  - `subject`: Email subject
  - `body`: Email body (text/html)
  - `timestamp`: DateTime
  - `raw`: Optional raw email data

### Part 2: API Server

#### Framework
- **axum** - Lightweight, async web framework with excellent WebSocket support

#### REST Endpoints
- `GET /api/emails/:address` - Get all emails for an address
- `GET /api/email/:id` - Get specific email by ID

#### WebSocket
- `WS /api/ws/:address` - WebSocket connection for live email updates
- Event broadcasting when new emails arrive
- Pub/sub pattern for real-time notifications

#### Features
- CORS support for frontend
- Static file serving for frontend
- JSON responses

### Part 3: Frontend

#### Technology
- Simple HTML/CSS/JavaScript (vanilla)
- Can be upgraded to React/Vue later if needed

#### Features
- **Input**: Email address field
- **Email List**: Show subject, from, timestamp
- **Email Detail**: Full email view
- **Real-time Updates**: WebSocket integration
- **Modern UI**: Clean, responsive design

#### Pages
- Single-page application
- Email inbox view
- Email detail modal/view

## Project Structure

```
src/
├── main.rs              # Entry point, starts both SMTP & API servers
├── smtp/
│   ├── mod.rs          # SMTP server implementation
│   └── parser.rs       # Email parsing logic
├── storage/
│   ├── mod.rs          # StorageBackend trait definition
│   ├── sqlite.rs       # SQLite implementation
│   └── models.rs       # Email data models
├── api/
│   ├── mod.rs          # API router and setup
│   ├── handlers.rs     # HTTP request handlers
│   └── websocket.rs    # WebSocket connection handling
└── frontend/
    └── static/         # HTML, CSS, JS files
        ├── index.html
        ├── style.css
        └── app.js
```

## Dependencies

### Core
- `tokio` - Async runtime
- `axum` - Web framework
- `tower` - Middleware
- `tower-http` - CORS, static files

### Storage
- `sqlx` - Async SQLite with compile-time checks
- `uuid` - Email ID generation

### Email Handling
- `mailin-embedded` or `smtp-server` - SMTP server
- `mail-parser` - Email parsing

### Serialization
- `serde` - Serialization framework
- `serde_json` - JSON support

### Other
- `chrono` or `time` - Timestamp handling
- `anyhow` - Error handling

## Key Features

✅ **No Authentication** - Temporary mail concept, no passwords
✅ **Accept All** - Accept emails to any address
✅ **Swappable Storage** - Backend trait allows different storage implementations
✅ **Real-time Updates** - WebSocket for instant email delivery notifications
✅ **Simple Frontend** - Single-page, easy to use
✅ **Auto-expire** - Emails can auto-delete after configurable time (optional feature)

## Configuration

### Environment Variables / Config File
- `SMTP_PORT` - SMTP server port (default: 2525)
- `API_PORT` - API server port (default: 3000)
- `DATABASE_URL` - SQLite database path (default: ./emails.db)
- `EMAIL_RETENTION_HOURS` - How long to keep emails (optional)

## Development Phases

### Phase 1: Core Infrastructure
1. Set up project structure
2. Define `StorageBackend` trait and models
3. Implement SQLite backend

### Phase 2: SMTP Server
1. Implement SMTP server
2. Email parsing
3. Integration with storage backend

### Phase 3: API Server
1. Set up axum routes
2. Implement REST endpoints
3. WebSocket handling
4. Event broadcasting

### Phase 4: Frontend
1. Create HTML/CSS layout
2. Implement email list view
3. Email detail view
4. WebSocket integration

### Phase 5: Integration & Testing
1. Connect all components
2. Test email flow end-to-end
3. WebSocket real-time updates
4. Error handling

## Future Enhancements (Optional)
- Email search/filtering
- Multiple storage backends (Redis, PostgreSQL)
- Attachments download
- Email forwarding
- API authentication (for production use)
- Docker containerization
- Rate limiting

