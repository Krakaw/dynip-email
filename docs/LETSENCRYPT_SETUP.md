# Let's Encrypt SSL Setup for SMTP

This guide explains how to set up Let's Encrypt SSL certificates for secure SMTP (STARTTLS/TLS).

## Prerequisites

- A domain name pointing to your server (e.g., `mail.yourdomain.com`)
- Root or sudo access to your server
- Ports 80 and 443 accessible (for Let's Encrypt verification)
- Port 465 or 587 for secure SMTP (optional, can use 25 with STARTTLS)

## Step 1: Install Certbot

### Ubuntu/Debian
```bash
sudo apt update
sudo apt install certbot
```

### CentOS/RHEL
```bash
sudo yum install certbot
```

### macOS (for testing)
```bash
brew install certbot
```

## Step 2: Obtain Let's Encrypt Certificates

### Standalone Method (if no web server is running)
```bash
sudo certbot certonly --standalone -d mail.yourdomain.com
```

### Webroot Method (if you have a web server)
```bash
sudo certbot certonly --webroot -w /var/www/html -d mail.yourdomain.com
```

### DNS Challenge (recommended for mail servers)
```bash
sudo certbot certonly --manual --preferred-challenges dns -d mail.yourdomain.com
```

Follow the prompts and add the TXT record to your DNS.

## Step 3: Locate Your Certificates

After successful verification, your certificates will be in:
```
/etc/letsencrypt/live/mail.yourdomain.com/
├── fullchain.pem  (certificate chain)
├── privkey.pem    (private key)
├── cert.pem       (certificate only)
└── chain.pem      (intermediate certificates)
```

## Step 4: Set Permissions

The application needs read access to the certificates:

```bash
# Option 1: Add your user to the certbot/letsencrypt group
sudo usermod -a -G ssl-cert $USER

# Option 2: Copy certificates to app directory (less secure, needs renewal script)
sudo mkdir -p /opt/dynip-email/certs
sudo cp /etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem /opt/dynip-email/certs/
sudo cp /etc/letsencrypt/live/mail.yourdomain.com/privkey.pem /opt/dynip-email/certs/
sudo chown -R your-app-user:your-app-user /opt/dynip-email/certs
sudo chmod 600 /opt/dynip-email/certs/privkey.pem

# Option 3: Run as root (not recommended for production)
```

## Step 5: Configure the Application

Create or update your `.env` file:

```env
SMTP_PORT=587
DOMAIN_NAME=mail.yourdomain.com

# Enable SSL
SMTP_SSL_ENABLED=true

# Certificate paths
SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem
SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/mail.yourdomain.com/privkey.pem
```

## Step 6: Update DNS Records

Add these DNS records for your domain:

```
# MX record (if you want to receive email)
@  IN  MX  10  mail.yourdomain.com.

# A record for mail server
mail  IN  A  YOUR_SERVER_IP

# Optional: SPF record
@  IN  TXT  "v=spf1 mx ~all"
```

## Step 7: Configure Firewall

Allow SMTP ports:

```bash
# For UFW (Ubuntu)
sudo ufw allow 587/tcp   # Submission port with STARTTLS
sudo ufw allow 465/tcp   # SMTPS port (optional)

# For firewalld (CentOS/RHEL)
sudo firewall-cmd --permanent --add-port=587/tcp
sudo firewall-cmd --permanent --add-port=465/tcp
sudo firewall-cmd --reload
```

## Step 8: Start the Application

```bash
cargo run --release
```

You should see:
```
📝 Configuration:
  SMTP Port: 587
  SMTP SSL: Enabled
  Domain: mail.yourdomain.com
```

## Step 9: Test SMTP SSL

### Test with OpenSSL
```bash
# Test STARTTLS on port 587
openssl s_client -starttls smtp -connect mail.yourdomain.com:587

# Test direct SSL on port 465
openssl s_client -connect mail.yourdomain.com:465
```

### Test with swaks
```bash
swaks --to test@example.com \
      --from sender@yourdomain.com \
      --server mail.yourdomain.com:587 \
      --tls \
      --auth-user user \
      --auth-password pass
```

### Test with Python
```python
import smtplib
import ssl

context = ssl.create_default_context()

with smtplib.SMTP("mail.yourdomain.com", 587) as server:
    server.starttls(context=context)
    server.login("user", "pass")
    server.send_message(msg)
```

## Certificate Renewal

Let's Encrypt certificates expire after 90 days. Set up automatic renewal:

### Create Renewal Hook

Create `/etc/letsencrypt/renewal-hooks/deploy/reload-mail.sh`:

```bash
#!/bin/bash
# Reload the mail server after certificate renewal

# If using systemd
systemctl reload dynip-email

# Or if running manually, you'll need to restart the process
# pkill -HUP dynip-email
```

Make it executable:
```bash
sudo chmod +x /etc/letsencrypt/renewal-hooks/deploy/reload-mail.sh
```

### Setup Auto-Renewal

```bash
# Test renewal
sudo certbot renew --dry-run

# Enable automatic renewal (usually already enabled)
sudo systemctl enable certbot.timer
sudo systemctl start certbot.timer

# Or add to crontab
sudo crontab -e
# Add: 0 0 * * * certbot renew --quiet --deploy-hook /etc/letsencrypt/renewal-hooks/deploy/reload-mail.sh
```

## Troubleshooting

### Permission Denied Error
```
Error: Permission denied (os error 13)
```

Solution: Ensure your user has read permissions for the certificate files.

### Certificate Not Found
```
Error: No such file or directory
```

Solution: Verify the certificate paths in your `.env` file match the actual certificate location.

### Certificate Expired
```
Error: certificate has expired
```

Solution: Renew your certificates:
```bash
sudo certbot renew --force-renewal
```

### STARTTLS Not Working

Check if the certificate chain is complete:
```bash
openssl s_client -starttls smtp -connect mail.yourdomain.com:587 -showcerts
```

Ensure you're using `fullchain.pem`, not just `cert.pem`.

## Production Deployment

For production, consider:

1. **Use a systemd service** to manage the application
2. **Set up monitoring** for certificate expiration
3. **Configure firewall** properly
4. **Use a reverse proxy** (nginx/caddy) for the web interface
5. **Implement rate limiting** for SMTP
6. **Set up logging** and monitoring
7. **Regular backups** of the database

## API SSL Termination (Separate)

Since API SSL is handled elsewhere, set up a reverse proxy:

### Nginx Example

```nginx
server {
    listen 443 ssl http2;
    server_name mail.yourdomain.com;
    
    ssl_certificate /etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/mail.yourdomain.com/privkey.pem;
    
    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    
    location /api/ws/ {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### Caddy Example

```
mail.yourdomain.com {
    reverse_proxy localhost:3000
}
```

Caddy automatically handles Let's Encrypt certificates!

## Security Best Practices

1. **Keep certificates secure**: Never commit certificate files to version control
2. **Restrict permissions**: Certificate files should be readable only by the application user
3. **Monitor expiration**: Set up alerts for certificate expiration
4. **Use strong ciphers**: The application uses modern TLS 1.2/1.3 by default
5. **Regular updates**: Keep certbot and the application updated

## Support

For issues specific to Let's Encrypt: https://community.letsencrypt.org/
For application issues: Check the logs with `RUST_LOG=debug cargo run`

