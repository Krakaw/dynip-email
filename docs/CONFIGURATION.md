# Configuration Guide

This guide explains all configuration options for the temporary mail server.

## Configuration Methods

The server can be configured in two ways:

1. **Environment Variables**: Set directly in your shell or deployment environment
2. **.env File**: Create a `.env` file in the project root (recommended for local development)

The server uses [dotenvy](https://github.com/allan2/dotenvy) to automatically load `.env` files.

**Quick Start**: Copy the example configuration file:
```bash
cp env.example .env
```

## Configuration Options

### Server Ports

#### SMTP_PORT
- **Default**: `2525`
- **Description**: Port for the SMTP server to listen on
- **Values**: Any valid port number (1-65535)
- **Common Choices**:
  - `25`: Standard SMTP (requires root/admin)
  - `587`: Submission port (recommended for authenticated SMTP)
  - `2525`: Alternative submission port (no special privileges needed)
  - `465`: SMTPS (deprecated but still used)

```env
SMTP_PORT=587
```

#### API_PORT
- **Default**: `3000`
- **Description**: Port for the HTTP API and web interface
- **Note**: Runs HTTP only - use a reverse proxy for HTTPS

```env
API_PORT=3000
```

### Database

#### DATABASE_URL
- **Default**: `sqlite:emails.db`
- **Description**: Database connection string
- **Format**: `sqlite:path/to/database.db`

```env
DATABASE_URL=sqlite:/var/lib/dynip-email/emails.db
```

### Domain Configuration

#### DOMAIN_NAME
- **Default**: `tempmail.local`
- **Description**: Domain name used in SMTP greeting and hostname
- **Important**: Should match your server's domain name for proper email delivery

```env
DOMAIN_NAME=mail.yourdomain.com
```

### SMTP SSL/TLS (Let's Encrypt)

#### SMTP_SSL_ENABLED
- **Default**: `false`
- **Description**: Enable SSL/TLS for SMTP using Let's Encrypt certificates
- **Values**: `true` or `false`
- **Note**: Requires `SMTP_SSL_CERT_PATH` and `SMTP_SSL_KEY_PATH` to be set

```env
SMTP_SSL_ENABLED=true
```

#### SMTP_SSL_CERT_PATH
- **Default**: None (required if SSL enabled)
- **Description**: Path to the SSL certificate file (fullchain.pem)
- **Common Location**: `/etc/letsencrypt/live/yourdomain.com/fullchain.pem`

```env
SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem
```

#### SMTP_SSL_KEY_PATH
- **Default**: None (required if SSL enabled)
- **Description**: Path to the SSL private key file (privkey.pem)
- **Common Location**: `/etc/letsencrypt/live/yourdomain.com/privkey.pem`
- **Security**: Ensure this file has restrictive permissions (600)

```env
SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/mail.yourdomain.com/privkey.pem
```

### Email Filtering

#### REJECT_NON_DOMAIN_EMAILS
- **Default**: `false`
- **Description**: Reject emails that are not addressed to the defined DOMAIN_NAME
- **Values**: `true` or `false`
- **Note**: When true, only emails to @DOMAIN_NAME will be accepted

```env
REJECT_NON_DOMAIN_EMAILS=false
```

### Email Retention

#### EMAIL_RETENTION_HOURS
- **Default**: None (emails never auto-delete)
- **Description**: Automatically delete emails older than this many hours
- **Values**: Any positive integer
- **Note**: Fully implemented with hourly cleanup task that runs automatically

```env
EMAIL_RETENTION_HOURS=24
```

### Logging

#### RUST_LOG
- **Default**: `info`
- **Description**: Set the log level for the application
- **Values**:
  - `error`: Only errors
  - `warn`: Warnings and errors
  - `info`: Informational messages (recommended)
  - `debug`: Detailed debugging information
  - `trace`: Very verbose debugging

```env
RUST_LOG=info
```

You can also set per-module logging:
```env
RUST_LOG=info,dynip_email::smtp=debug
```

## Example Configurations

### Development (Local Testing)

```.env
# Development configuration
SMTP_PORT=2525
API_PORT=3000
DATABASE_URL=sqlite:emails.db
DOMAIN_NAME=localhost
SMTP_SSL_ENABLED=false
RUST_LOG=debug
```

### Production (Basic)

```.env
# Production configuration without SSL
SMTP_PORT=2525
API_PORT=3000
DATABASE_URL=sqlite:/var/lib/dynip-email/emails.db
DOMAIN_NAME=mail.yourdomain.com
SMTP_SSL_ENABLED=false
RUST_LOG=info
EMAIL_RETENTION_HOURS=48
```

### Production (With Let's Encrypt SSL)

```.env
# Production configuration with SSL
SMTP_PORT=587
API_PORT=3000
DATABASE_URL=sqlite:/var/lib/dynip-email/emails.db
DOMAIN_NAME=mail.yourdomain.com

# Let's Encrypt SSL for SMTP
SMTP_SSL_ENABLED=true
SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem
SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/mail.yourdomain.com/privkey.pem

RUST_LOG=info
EMAIL_RETENTION_HOURS=72
```

## Security Considerations

### File Permissions

When using SSL certificates:

```bash
# Certificate files should be readable
chmod 644 /etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem

# Private key should be readable only by owner
chmod 600 /etc/letsencrypt/live/mail.yourdomain.com/privkey.pem

# Ensure the application user can read them
chown root:ssl-cert /etc/letsencrypt/live/mail.yourdomain.com/*
usermod -a -G ssl-cert your-app-user
```

### Environment Variables

- Never commit `.env` files to version control
- Use secret management systems in production (e.g., AWS Secrets Manager, HashiCorp Vault)
- Rotate SSL certificates before expiration (Let's Encrypt: 90 days)

### Database Security

```bash
# Secure the database file
chmod 600 /var/lib/dynip-email/emails.db
chown your-app-user:your-app-user /var/lib/dynip-email/emails.db
```

## Configuration Validation

The server validates configuration on startup:

```
✅ Valid: All required settings present
❌ Invalid: Prints error and exits

Examples of validation errors:
- SMTP_SSL_ENABLED=true but certificate paths not set
- Invalid port numbers
- Inaccessible database path
- Missing SSL certificate files
```

## Loading Order

Configuration is loaded in this order (later overrides earlier):

1. Default values (hardcoded)
2. `.env` file in current directory
3. Environment variables

Example:
```bash
# .env file has SMTP_PORT=2525
# But you can override it:
SMTP_PORT=587 cargo run
```

## Troubleshooting

### SSL Certificate Errors

**Error**: `Permission denied reading certificate`
```bash
# Fix permissions
sudo chmod 644 /etc/letsencrypt/live/*/fullchain.pem
sudo chmod 600 /etc/letsencrypt/live/*/privkey.pem
```

**Error**: `Certificate not found`
```bash
# Verify path
ls -la /etc/letsencrypt/live/mail.yourdomain.com/
```

### Port Conflicts

**Error**: `Address already in use`
```bash
# Find what's using the port
lsof -i :2525

# Use a different port
SMTP_PORT=2526 cargo run
```

### Database Issues

**Error**: `unable to open database file`
```bash
# Ensure directory exists
mkdir -p /var/lib/dynip-email

# Fix permissions
chmod 755 /var/lib/dynip-email
```

## Additional Resources

- [Let's Encrypt Setup Guide](LETSENCRYPT_SETUP.md)
- [Production Deployment Guide](README.md#production-deployment)
- [API Documentation](README.md#api-endpoints)

