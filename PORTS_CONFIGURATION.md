# SMTP Ports Configuration

## Overview

The server now supports **three fully configurable SMTP ports** via environment variables:

## Port Configuration

### 1. Non-TLS Port (Always Listening)
- **Environment Variable**: `SMTP_PORT`
- **Default**: `2525`
- **Purpose**: Plain SMTP without encryption
- **Usage**: Legacy systems, local testing, or when encryption is handled elsewhere (VPN, private network)
- **Always Available**: Yes - this port listens regardless of SSL configuration

### 2. STARTTLS Port (SSL Enabled Only)
- **Environment Variable**: `SMTP_STARTTLS_PORT`
- **Default**: `587`
- **Purpose**: SMTP with explicit TLS upgrade (STARTTLS command)
- **Usage**: Modern email clients (recommended method)
- **Available When**: `SMTP_SSL_ENABLED=true`

### 3. SMTPS Port (SSL Enabled Only)
- **Environment Variable**: `SMTP_SSL_PORT`
- **Default**: `465`
- **Purpose**: SMTP with implicit TLS (TLS from connection start)
- **Usage**: Alternative secure method, some older email clients
- **Available When**: `SMTP_SSL_ENABLED=true`

## Configuration Examples

### Example 1: Development (No SSL)

```env
# .env file
SMTP_PORT=2525
API_PORT=3000
SMTP_SSL_ENABLED=false
```

**Result**: Server listens **only** on port 2525 (non-TLS)

### Example 2: Production with Standard Ports

```env
# .env file
SMTP_PORT=25
SMTP_STARTTLS_PORT=587
SMTP_SSL_PORT=465
SMTP_SSL_ENABLED=true
SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem
SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/mail.yourdomain.com/privkey.pem
DOMAIN_NAME=mail.yourdomain.com
```

**Result**: Server listens on **three ports**:
- Port 25 (non-TLS)
- Port 587 (STARTTLS)
- Port 465 (SMTPS)

### Example 3: Custom Ports

```env
# .env file
SMTP_PORT=2525
SMTP_STARTTLS_PORT=5870
SMTP_SSL_PORT=4650
SMTP_SSL_ENABLED=true
SMTP_SSL_CERT_PATH=/path/to/cert.pem
SMTP_SSL_KEY_PATH=/path/to/key.pem
```

**Result**: Server listens on **three custom ports**:
- Port 2525 (non-TLS)
- Port 5870 (STARTTLS)
- Port 4650 (SMTPS)

## Startup Output

### Without SSL:
```
📝 Configuration:
  SMTP Port (non-TLS): 2525
  SMTP SSL: Disabled
...
✅ SMTP server started on port 2525 (non-TLS only)
📬 SMTP server listening on port 2525 (non-TLS only)
```

### With SSL:
```
📝 Configuration:
  SMTP Port (non-TLS): 2525
  SMTP Port (STARTTLS): 587
  SMTP Port (SMTPS): 465
  SMTP SSL: Enabled (Let's Encrypt)
...
✅ SMTP servers started on ports: 2525 (non-TLS), 587 (STARTTLS), 465 (SMTPS)
📬 SMTP servers listening on:
   • Port 2525 (non-TLS) - standard SMTP
   • Port 587 (STARTTLS) - secure submission
   • Port 465 (SMTPS) - implicit TLS
🔒 SSL/TLS enabled with Let's Encrypt certificates
```

## Port Usage Recommendations

### Port 25 (Non-TLS on SMTP_PORT)
- **Standard SMTP port**
- Requires root/admin privileges on Linux/Unix
- Use for: Server-to-server email (MTA)
- Security: Consider using STARTTLS on port 587 instead

### Port 587 (STARTTLS)
- **Recommended for email clients**
- Submission port with STARTTLS
- Modern and widely supported
- Best for: Email clients (Outlook, Thunderbird, etc.)

### Port 465 (SMTPS)
- **Legacy but still used**
- Implicit TLS from connection start
- Use for: Clients that don't support STARTTLS
- Note: Was deprecated but has been re-standardized

### Port 2525 (Alternative Non-TLS)
- **No special privileges required**
- Good for: Development, testing, non-standard deployments
- Often used when port 25 is blocked or unavailable

## Firewall Configuration

When SSL is enabled, ensure all three ports are accessible:

```bash
# UFW (Ubuntu)
sudo ufw allow 2525/tcp  # or your SMTP_PORT
sudo ufw allow 587/tcp   # or your SMTP_STARTTLS_PORT
sudo ufw allow 465/tcp   # or your SMTP_SSL_PORT

# firewalld (CentOS/RHEL)
sudo firewall-cmd --permanent --add-port=2525/tcp
sudo firewall-cmd --permanent --add-port=587/tcp
sudo firewall-cmd --permanent --add-port=465/tcp
sudo firewall-cmd --reload
```

## Testing Ports

### Test Non-TLS Port
```bash
telnet localhost 2525
# or
nc localhost 2525
```

### Test STARTTLS Port
```bash
openssl s_client -starttls smtp -connect localhost:587
```

### Test SMTPS Port
```bash
openssl s_client -connect localhost:465
```

## Complete .env Template

```env
# ============================================================================
# SMTP Ports - All three are configurable
# ============================================================================

# Non-TLS port (always active)
SMTP_PORT=2525

# STARTTLS port (active when SMTP_SSL_ENABLED=true)
SMTP_STARTTLS_PORT=587

# SMTPS port (active when SMTP_SSL_ENABLED=true)
SMTP_SSL_PORT=465

# ============================================================================
# SSL/TLS Configuration
# ============================================================================

SMTP_SSL_ENABLED=true
SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem
SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/mail.yourdomain.com/privkey.pem
DOMAIN_NAME=mail.yourdomain.com

# ============================================================================
# Other Configuration
# ============================================================================

API_PORT=3000
DATABASE_URL=sqlite:emails.db
RUST_LOG=info
```

## Benefits of This Approach

✅ **Maximum Compatibility**: Support for all connection types
✅ **Fully Configurable**: Every port can be customized
✅ **Always Non-TLS**: Plain SMTP always available for testing/legacy
✅ **Secure Options**: Both STARTTLS and SMTPS when SSL is enabled
✅ **Flexible Deployment**: Works in any environment with custom ports

## Security Considerations

1. **Non-TLS Port**: Always available but unencrypted
   - Use only on trusted networks
   - Consider firewall rules to restrict access
   - Good for local/internal testing

2. **STARTTLS (Port 587)**: Recommended for production
   - Starts plain but upgrades to TLS
   - Widely supported by all modern email clients
   - Allows opportunistic encryption

3. **SMTPS (Port 465)**: Alternative secure method
   - TLS from connection start
   - No plaintext phase
   - Some clients prefer this method

## Summary

All three SMTP ports are now **fully configurable via .env file**:

| Variable | Default | Always Active? | SSL Required? |
|----------|---------|----------------|---------------|
| `SMTP_PORT` | 2525 | ✅ Yes | ❌ No |
| `SMTP_STARTTLS_PORT` | 587 | ❌ No | ✅ Yes |
| `SMTP_SSL_PORT` | 465 | ❌ No | ✅ Yes |

This provides maximum flexibility while maintaining security best practices.

