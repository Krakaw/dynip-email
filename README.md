# <img src="static/logo.svg" width="56"  /> Temporary Mail Server

A lightweight, temporary mail server that accepts emails to any address with a modern web interface and real-time updates via WebSocket.

## Features

✨ **Accept All Emails** - No validation, accepts emails to any address  
🔄 **Real-time Updates** - WebSocket integration for instant email notifications  
💾 **Flexible Storage** - Swappable backend storage (SQLite by default)  
🎨 **Modern UI** - Clean, responsive web interface with webhook management  
🚀 **Lightweight** - Minimal dependencies, fast performance  
📱 **No Registration** - Just enter any email address to view messages  
🗑️ **Auto-Cleanup** - Configurable email retention with automatic deletion  
🔔 **Live Notifications** - Real-time WebSocket updates for email deletions  
🔗 **Webhook Support** - Per-mailbox webhook events for email arrivals, deletions, and reads  
🤖 **MCP Integration** - Model Context Protocol server for LLM/AI assistant integration  
🔒 **Password Protection** - First-claim model to lock mailboxes with passwords  
📬 **IMAP Server** - Retrieve emails via standard IMAP protocol with authentication  

## Quick Start

### Prerequisites

- Rust 1.90+ (install from [rustup.rs](https://rustup.rs))

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

### Server Startup

The server will start with:
- **Web Interface**: http://localhost:3000
- **SMTP Server**: localhost:2525

### Configuration

Create a `.env` file or set environment variables to customize the server:

```bash
# Copy the example file
cp env.example .env

# Edit the configuration
nano .env
```

Key configuration options:

| Variable | Default | Description |
|----------|---------|-------------|
| `SMTP_PORT` | 2525 | SMTP server port (non-TLS, always listening) |
| `SMTP_STARTTLS_PORT` | 587 | STARTTLS port (when SSL enabled) |
| `SMTP_SSL_PORT` | 465 | SMTPS port (when SSL enabled) |
| `API_PORT` | 3000 | API/Web server port (HTTP only) |
| `DATABASE_URL` | sqlite:emails.db | Database connection string |
| `DOMAIN_NAME` | tempmail.local | Domain name for SMTP greeting |
| `SMTP_SSL_ENABLED` | false | Enable Let's Encrypt SSL for SMTP |
| `SMTP_SSL_CERT_PATH` | - | Path to SSL certificate (fullchain.pem) |
| `SMTP_SSL_KEY_PATH` | - | Path to SSL private key (privkey.pem) |
| `EMAIL_RETENTION_HOURS` | - | Auto-delete emails older than X hours (optional) |
| `REJECT_NON_DOMAIN_EMAILS` | false | Reject emails not addressed to DOMAIN_NAME |
| `IMAP_ENABLED` | false | Enable IMAP server for email retrieval |
| `IMAP_PORT` | 143 | IMAP server port |
| `RUST_LOG` | info | Log level (trace, debug, info, warn, error) |

For detailed configuration options, see the [Configuration Guide](docs/CONFIGURATION.md).

**Note**: When `SMTP_SSL_ENABLED=true`, the server listens on **three ports**:
- `SMTP_PORT` (non-TLS, always available)
- `SMTP_STARTTLS_PORT` (STARTTLS - recommended)
- `SMTP_SSL_PORT` (SMTPS - implicit TLS)

**Note**: API SSL termination should be handled by a reverse proxy (nginx, caddy, traefik, etc.)

## DNS Configuration for Email Delivery

To receive emails from external senders, you need to configure DNS records for your domain. Essential records include:

- **MX Record**: Points to your mail server
- **A Record**: Resolves your mail server hostname to IP
- **PTR Record**: Reverse DNS for your server IP
- **SPF Record**: Authorizes your server to send emails
- **DKIM/DMARC Records**: Optional but recommended for deliverability

For detailed DNS setup instructions, see the [Let's Encrypt Setup Guide](docs/LETSENCRYPT_SETUP.md).

### Production Configuration

For production deployment, update your `.env` file:

```env
# Set your domain name
DOMAIN_NAME=mail.yourdomain.com

# Enable SSL/TLS for better deliverability
SMTP_SSL_ENABLED=true
SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem
SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/mail.yourdomain.com/privkey.pem

# Use standard SMTP ports
SMTP_PORT=25
SMTP_STARTTLS_PORT=587
SMTP_SSL_PORT=465
```

### Important Notes

- **Port 25**: Most ISPs block port 25 for residential connections. You may need a VPS or dedicated server.
- **Firewall**: Ensure your server's firewall allows connections on ports 25, 587, and 465.
- **Email Reputation**: New mail servers may face deliverability issues initially.

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
python3 scripts/test_email.py
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
- `POST /api/webhooks` - Create a new webhook
- `GET /api/webhooks/:address` - List webhooks for a mailbox
- `GET /api/webhook/:id` - Get webhook details
- `PUT /api/webhook/:id` - Update webhook
- `DELETE /api/webhook/:id` - Delete webhook
- `POST /api/webhook/:id/test` - Test webhook

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

## Webhook Integration

Configure webhooks to receive real-time notifications for email events:

### Supported Events
- **Email Arrival**: When a new email is received
- **Email Deletion**: When an email is deleted (retention policy)
- **Email Read**: When an email is viewed (optional)

### Webhook Configuration
1. Navigate to the web interface
2. Enter an email address to load the mailbox
3. Click the "Webhooks" tab
4. Add webhook URLs and select events
5. Test webhooks to verify delivery

### Webhook Payload Example
```json
{
  "event": "arrival",
  "mailbox": "user@example.com",
  "webhook_id": "webhook-uuid",
  "timestamp": "2024-01-01T00:00:00Z",
  "email": {
    "id": "email-uuid",
    "to": "user@example.com",
    "from": "sender@example.com",
    "subject": "Email Subject",
    "body": "Email content",
    "timestamp": "2024-01-01T00:00:00Z",
    "attachments": 0
  }
}
```

## IMAP Server

Enable IMAP access to retrieve emails using standard email clients:

### Configuration
```bash
IMAP_ENABLED=true
IMAP_PORT=143
```

### Authentication
IMAP authentication uses the same mailbox passwords as the web interface:
1. First claim a mailbox by visiting the web UI
2. Set a password for the mailbox
3. Use the mailbox address (without @domain) as username and your password

### Example Client Configuration
```
Server: your-server.com
Port: 143
Username: myaddress (or myaddress@domain.com)
Password: your-mailbox-password
Security: None (or STARTTLS if SSL enabled)
```

### Supported Commands
- `LOGIN` - Authenticate with username/password
- `LIST` / `LSUB` - List mailboxes
- `SELECT` / `EXAMINE` - Select a mailbox
- `FETCH` - Retrieve email content
- `SEARCH` - Search emails
- `UID FETCH` / `UID SEARCH` - UID-based operations
- `CLOSE` / `LOGOUT` - Close connection

## MCP (Model Context Protocol) Integration

Enable AI assistant integration with the MCP server:

### Configuration
```bash
MCP_ENABLED=true
MCP_PORT=3001
```

### MCP Tools Available
- `list_emails` - List emails for a mailbox
- `read_email` - Get email by ID
- `delete_email` - Delete email by ID
- `create_webhook` - Create webhook for mailbox
- `list_webhooks` - List webhooks for mailbox
- `delete_webhook` - Delete webhook
- `test_webhook` - Test webhook delivery

### MCP Resources
- `email://{email_id}` - Access email content
- `webhook://{webhook_id}` - Access webhook configuration

See [MCP Integration Guide](docs/MCP_INTEGRATION.md) for detailed usage.

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
├── webhooks/
│   └── mod.rs          # Webhook handling
├── mcp/
│   └── mod.rs          # MCP server
└── config.rs           # Configuration management

static/                 # Frontend files
├── index.html          # Web interface
├── app.js              # JavaScript
├── style.css           # Styling
└── logo.svg            # Logo
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

## Documentation

For detailed guides and technical documentation, see the `/docs` folder:

- **[Configuration Guide](docs/CONFIGURATION.md)** - Comprehensive reference for all configuration options
- **[Email Retention](docs/EMAIL_RETENTION.md)** - Automatic email cleanup and retention policies with WebSocket notifications
- **[Let's Encrypt Setup](docs/LETSENCRYPT_SETUP.md)** - Step-by-step guide for SSL/TLS configuration
- **[Webhooks](docs/WEBHOOKS.md)** - Webhook configuration and debugging
- **[MCP Integration](docs/MCP_INTEGRATION.md)** - Model Context Protocol server for LLM integration
- **[Docker Deployment](docs/DOCKER_DEPLOYMENT.md)** - Docker and Docker Compose deployment
- **[Systemd Service](docs/SYSTEMD_SERVICE.md)** - Running as a systemd service
- **[Mailbox Password Protection](docs/MAILBOX_PASSWORD_PROTECTION.md)** - First-claim password protection for mailboxes

## Future Enhancements

- [ ] Email search and filtering
- [ ] Multiple storage backends (PostgreSQL, Redis)
- [ ] Attachment downloads
- [ ] Email forwarding
- [ ] API authentication
- [ ] Docker containerization
- [ ] Rate limiting
- [x] Auto-expire old emails (implemented via EMAIL_RETENTION_HOURS)
- [x] Mailbox password protection (implemented via first-claim model)

## License

MIT License - See LICENSE file for details

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

