# DynIP Email Systemd Service

This directory contains files for running the DynIP Email application as a native systemd service on Linux systems.

## Files

- `dynip-email.service` - Systemd service unit file
- `install-service.sh` - Automated installation script
- `SYSTEMD_SERVICE.md` - This documentation

## Quick Start

1. **Build the application:**
   ```bash
   cargo build --release
   ```

2. **Install the service:**
   ```bash
   sudo ./scripts/install-service.sh
   ```

3. **Start the service:**
   ```bash
   sudo systemctl start dynip-email
   ```

4. **Check status:**
   ```bash
   sudo systemctl status dynip-email
   ```

## Manual Installation

If you prefer to install manually:

1. **Create service user:**
   ```bash
   sudo useradd --system --no-create-home --shell /bin/false dynip-email
   ```

2. **Create directories:**
   ```bash
   sudo mkdir -p /opt/dynip-email
   sudo mkdir -p /var/lib/dynip-email
   sudo chown -R dynip-email:dynip-email /opt/dynip-email
   sudo chown -R dynip-email:dynip-email /var/lib/dynip-email
   ```

3. **Copy binary:**
   ```bash
   sudo cp target/release/dynip-email /opt/dynip-email/
   sudo chown dynip-email:dynip-email /opt/dynip-email/dynip-email
   sudo chmod +x /opt/dynip-email/dynip-email
   ```

4. **Install service file:**
   ```bash
   sudo cp scripts/dynip-email.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable dynip-email
   ```

## Configuration

The service file includes default environment variables. To customize:

1. **Edit the service file:**
   ```bash
   sudo nano /etc/systemd/system/dynip-email.service
   ```

2. **Reload and restart:**
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl restart dynip-email
   ```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level |
| `SMTP_PORT` | `2525` | Non-TLS SMTP port |
| `SMTP_STARTTLS_PORT` | `587` | STARTTLS port |
| `SMTP_SSL_PORT` | `465` | SMTPS port |
| `API_PORT` | `3000` | HTTP API port |
| `DATABASE_URL` | `sqlite:/var/lib/dynip-email/emails.db` | Database location |
| `DOMAIN_NAME` | `tempmail.local` | SMTP domain |
| `REJECT_NON_DOMAIN_EMAILS` | `false` | Reject non-domain emails |
| `SMTP_SSL_ENABLED` | `false` | Enable SSL/TLS |
| `EMAIL_RETENTION_HOURS` | `24` | Auto-delete emails after N hours |

### SSL/TLS Configuration

To enable SSL/TLS with Let's Encrypt certificates:

1. **Uncomment SSL environment variables in the service file:**
   ```ini
   Environment=SMTP_SSL_ENABLED=true
   Environment=SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem
   Environment=SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/mail.yourdomain.com/privkey.pem
   ```

2. **Update domain name:**
   ```ini
   Environment=DOMAIN_NAME=mail.yourdomain.com
   ```

3. **Reload and restart:**
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl restart dynip-email
   ```

## Service Management

### Basic Commands

```bash
# Start service
sudo systemctl start dynip-email

# Stop service
sudo systemctl stop dynip-email

# Restart service
sudo systemctl restart dynip-email

# Check status
sudo systemctl status dynip-email

# Enable auto-start on boot
sudo systemctl enable dynip-email

# Disable auto-start
sudo systemctl disable dynip-email
```

### Logs

```bash
# View recent logs
sudo journalctl -u dynip-email

# Follow logs in real-time
sudo journalctl -u dynip-email -f

# View logs from last boot
sudo journalctl -u dynip-email -b

# View logs with timestamps
sudo journalctl -u dynip-email --since "1 hour ago"
```

### Service Status

```bash
# Check if service is running
sudo systemctl is-active dynip-email

# Check if service is enabled
sudo systemctl is-enabled dynip-email

# Show service properties
sudo systemctl show dynip-email
```

## Security Features

The service file includes several security measures:

- **User isolation:** Runs as dedicated `dynip-email` user
- **No new privileges:** Prevents privilege escalation
- **Private tmp:** Isolated temporary directory
- **Protected system:** Read-only system directories
- **Protected home:** No access to user home directories
- **Limited file access:** Only specific paths are writable

## Troubleshooting

### Service won't start

1. **Check logs:**
   ```bash
   sudo journalctl -u dynip-email -n 50
   ```

2. **Check binary permissions:**
   ```bash
   ls -la /opt/dynip-email/dynip-email
   ```

3. **Check directory permissions:**
   ```bash
   ls -la /var/lib/dynip-email/
   ```

### Permission issues

1. **Fix ownership:**
   ```bash
   sudo chown -R dynip-email:dynip-email /opt/dynip-email
   sudo chown -R dynip-email:dynip-email /var/lib/dynip-email
   ```

2. **Fix permissions:**
   ```bash
   sudo chmod +x /opt/dynip-email/dynip-email
   sudo chmod 755 /opt/dynip-email
   sudo chmod 755 /var/lib/dynip-email
   ```

### Port conflicts

1. **Check if ports are in use:**
   ```bash
   sudo netstat -tlnp | grep -E ':(2525|3000|587|465)'
   ```

2. **Change ports in service file:**
   ```bash
   sudo nano /etc/systemd/system/dynip-email.service
   ```

### Database issues

1. **Check database file:**
   ```bash
   ls -la /var/lib/dynip-email/emails.db
   ```

2. **Fix database permissions:**
   ```bash
   sudo chown dynip-email:dynip-email /var/lib/dynip-email/emails.db
   ```

## Uninstallation

To remove the service:

```bash
# Stop and disable service
sudo systemctl stop dynip-email
sudo systemctl disable dynip-email

# Remove service file
sudo rm /etc/systemd/system/dynip-email.service
sudo systemctl daemon-reload

# Remove application files
sudo rm -rf /opt/dynip-email

# Remove data (optional - backup first!)
sudo rm -rf /var/lib/dynip-email

# Remove user
sudo userdel dynip-email
```

## Production Considerations

### Firewall

Configure firewall rules for the required ports:

```bash
# UFW example
sudo ufw allow 2525/tcp  # SMTP
sudo ufw allow 3000/tcp  # API
sudo ufw allow 587/tcp   # STARTTLS (if SSL enabled)
sudo ufw allow 465/tcp   # SMTPS (if SSL enabled)
```

### Reverse Proxy

For production, use a reverse proxy (nginx, caddy) for HTTPS:

```nginx
# nginx example
server {
    listen 443 ssl;
    server_name mail.yourdomain.com;
    
    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Monitoring

Consider setting up monitoring for:
- Service health
- Disk space (database growth)
- Memory usage
- Log rotation

### Backup

Regularly backup the database:
```bash
# Backup database
sudo cp /var/lib/dynip-email/emails.db /backup/emails-$(date +%Y%m%d).db
```
