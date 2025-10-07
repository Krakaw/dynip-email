# 📧 Temporary Mail Server

A lightweight, temporary mail server that accepts emails to any address with a modern web interface and real-time updates via WebSocket.

## Features

✨ **Accept All Emails** - No validation, accepts emails to any address  
🔄 **Real-time Updates** - WebSocket integration for instant email notifications  
💾 **Flexible Storage** - Swappable backend storage (SQLite by default)  
🎨 **Modern UI** - Clean, responsive web interface  
🚀 **Lightweight** - Minimal dependencies, fast performance  
📱 **No Registration** - Just enter any email address to view messages  

## Quick Start

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))

### Installation & Running

1. Clone the repository:
```bash
git clone <repository-url>
cd dynip-email
```

2. Run the server:
```bash
cargo run --release
```

The server will start with:
- **Web Interface**: http://localhost:3000
- **SMTP Server**: localhost:2525

### Configuration

Set environment variables to customize the server:

```bash
# SMTP server port (default: 2525)
export SMTP_PORT=2525

# API/Web server port (default: 3000)
export API_PORT=3000

# SQLite database location (default: sqlite:emails.db)
export DATABASE_URL=sqlite:emails.db
```

## Usage

### Web Interface

1. Open your browser to http://localhost:3000
2. Enter any email address (e.g., `test@example.com`)
3. Click "Load Inbox"
4. Emails will appear in real-time as they arrive

### Sending Test Emails

#### Using the included test script:

```bash
# Run the Python test script (sends 3 test emails)
python3 test_email.py
```

#### Manual testing:

```bash
# Using swaks (SMTP test tool)
swaks --to test@example.com \
      --from sender@example.com \
      --server localhost:2525 \
      --body "Hello from temporary mail!"

# Using Python
python3 -c "
import smtplib
from email.mime.text import MIMEText

msg = MIMEText('This is a test email')
msg['Subject'] = 'Test Email'
msg['From'] = 'sender@example.com'
msg['To'] = 'test@localhost'

with smtplib.SMTP('localhost', 2525) as server:
    server.send_message(msg)
print('Email sent!')
"
```

## API Endpoints

### REST API

- `GET /api/emails/:address` - Get all emails for an address
- `GET /api/email/:id` - Get a specific email by ID

Example:
```bash
curl http://localhost:3000/api/emails/test@example.com
```

### WebSocket

- `WS /api/ws/:address` - Real-time email updates for an address

Example:
```javascript
const ws = new WebSocket('ws://localhost:3000/api/ws/test@example.com');
ws.onmessage = (event) => {
    const email = JSON.parse(event.data);
    console.log('New email:', email);
};
```

## Architecture

The project is organized into three main components:

### 1. Email Service (`src/smtp/`)
- SMTP server accepting emails on port 2525
- Email parsing using `mail-parser`
- Storage integration

### 2. API Server (`src/api/`)
- REST endpoints for email retrieval
- WebSocket support for real-time updates
- Static file serving for frontend

### 3. Storage Layer (`src/storage/`)
- `StorageBackend` trait for swappable implementations
- SQLite backend (default)
- Email data models

## Development

### Project Structure

```
src/
├── main.rs              # Application entry point
├── smtp/
│   ├── mod.rs          # SMTP server
│   └── parser.rs       # Email parsing
├── storage/
│   ├── mod.rs          # StorageBackend trait
│   ├── sqlite.rs       # SQLite implementation
│   └── models.rs       # Email data models
├── api/
│   ├── mod.rs          # API router
│   ├── handlers.rs     # REST endpoints
│   └── websocket.rs    # WebSocket handling
└── frontend/
    └── static/         # HTML, CSS, JS files
```

### Building

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run with logging
RUST_LOG=info cargo run
```

### Testing

Send test emails:

```bash
# Install swaks (Debian/Ubuntu)
sudo apt-get install swaks

# Send test email
swaks --to user@test.com \
      --from sender@example.com \
      --server localhost:2525 \
      --header "Subject: Test Email" \
      --body "This is a test message"
```

## Future Enhancements

- [ ] Email search and filtering
- [ ] Multiple storage backends (PostgreSQL, Redis)
- [ ] Attachment downloads
- [ ] Email forwarding
- [ ] API authentication
- [ ] Docker containerization
- [ ] Rate limiting
- [ ] Auto-expire old emails

## License

MIT License - See LICENSE file for details

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

