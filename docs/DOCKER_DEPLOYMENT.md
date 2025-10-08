# Docker Deployment Guide

This guide explains how to deploy dynip-email using Docker and Docker Compose.

## Quick Start

### 1. Basic Deployment (HTTP only)

```bash
# Clone the repository
git clone <repository-url>
cd dynip-email

# Build and start the services
docker-compose up -d

# Check logs
docker-compose logs -f dynip-email
```

The service will be available at:
- **Web Interface**: http://localhost:3000
- **SMTP Server**: localhost:2525

### 2. Production Deployment (with HTTPS)

For production deployment with SSL/TLS support:

```bash
# 1. Configure SSL certificates (Let's Encrypt)
# Mount your certificates in docker-compose.yml

# 2. Update environment variables
cp docker.env .env
# Edit .env and set:
# - SMTP_SSL_ENABLED=true
# - SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/yourdomain.com/fullchain.pem
# - SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/yourdomain.com/privkey.pem
# - DOMAIN_NAME=yourdomain.com

# 3. Start with nginx reverse proxy
docker-compose --profile nginx up -d
```

## Configuration

### Environment Variables

Copy `docker.env` to `.env` and customize:

```bash
cp docker.env .env
```

Key variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `SMTP_PORT` | 2525 | Non-TLS SMTP port |
| `SMTP_STARTTLS_PORT` | 587 | STARTTLS port (SSL) |
| `SMTP_SSL_PORT` | 465 | SMTPS port (SSL) |
| `API_PORT` | 3000 | Web interface port |
| `DATABASE_URL` | sqlite:/app/data/emails.db | Database location |
| `DOMAIN_NAME` | tempmail.local | SMTP domain |
| `SMTP_SSL_ENABLED` | false | Enable SSL/TLS |
| `RUST_LOG` | info | Log level |

### SSL/TLS Configuration

For production with SSL:

1. **Obtain SSL certificates** (Let's Encrypt recommended):
```bash
# Install certbot
sudo apt-get install certbot

# Get certificates
sudo certbot certonly --standalone -d mail.yourdomain.com
```

2. **Update docker-compose.yml**:
```yaml
volumes:
  - /etc/letsencrypt/live/yourdomain.com:/etc/letsencrypt/live/yourdomain.com:ro
```

3. **Set environment variables**:
```bash
SMTP_SSL_ENABLED=true
SMTP_SSL_CERT_PATH=/etc/letsencrypt/live/yourdomain.com/fullchain.pem
SMTP_SSL_KEY_PATH=/etc/letsencrypt/live/yourdomain.com/privkey.pem
DOMAIN_NAME=mail.yourdomain.com
```

## Docker Compose Services

### dynip-email
Main application service with:
- Web interface on port 3000
- SMTP server on ports 2525, 587, 465
- Persistent database storage
- Health checks

### nginx (optional)
Reverse proxy for HTTPS termination:
- HTTP to HTTPS redirect
- SSL/TLS termination
- Rate limiting
- WebSocket support

## Deployment Scenarios

### Development
```bash
# Simple development setup
docker-compose up -d dynip-email
```

### Production (No SSL)
```bash
# Production without SSL
docker-compose up -d dynip-email
```

### Production (With SSL)
```bash
# Production with SSL and nginx
docker-compose --profile nginx up -d
```

## Data Persistence

The application uses Docker volumes for data persistence:

- **Database**: `dynip-email-data` volume
- **Location**: `/app/data/emails.db` in container
- **Backup**: `docker run --rm -v dynip-email_dynip-email-data:/data -v $(pwd):/backup alpine tar czf /backup/emails-backup.tar.gz -C /data .`

## Monitoring

### Health Checks
```bash
# Check service health
docker-compose ps

# View logs
docker-compose logs -f dynip-email

# Check specific service
docker-compose logs dynip-email
```

### Database Access
```bash
# Access database directly
docker-compose exec dynip-email sqlite3 /app/data/emails.db

# Backup database
docker-compose exec dynip-email cp /app/data/emails.db /app/emails-backup.db
```

## Troubleshooting

### Common Issues

1. **Port conflicts**:
```bash
# Check port usage
netstat -tulpn | grep :3000
netstat -tulpn | grep :2525
```

2. **Permission issues**:
```bash
# Fix volume permissions
docker-compose exec dynip-email chown -R dynip-email:dynip-email /app/data
```

3. **SSL certificate issues**:
```bash
# Check certificate files
docker-compose exec dynip-email ls -la /etc/letsencrypt/live/yourdomain.com/
```

### Logs
```bash
# Application logs
docker-compose logs -f dynip-email

# Nginx logs (if using)
docker-compose logs -f nginx

# All services
docker-compose logs -f
```

## Security Considerations

1. **Firewall**: Only expose necessary ports
2. **SSL/TLS**: Use HTTPS in production
3. **Rate limiting**: Configure nginx rate limits
4. **Updates**: Keep Docker images updated
5. **Backups**: Regular database backups

## Scaling

For high-traffic deployments:

1. **Load balancing**: Use multiple dynip-email instances
2. **Database**: Consider PostgreSQL backend
3. **Caching**: Add Redis for session storage
4. **Monitoring**: Add Prometheus/Grafana

## Maintenance

### Updates
```bash
# Pull latest changes
git pull

# Rebuild and restart
docker-compose down
docker-compose build --no-cache
docker-compose up -d
```

### Cleanup
```bash
# Remove old images
docker image prune -a

# Remove unused volumes
docker volume prune
```

## Examples

### Send Test Email
```bash
# Using curl
curl -X POST http://localhost:3000/api/emails/test@example.com

# Using Python
python3 -c "
import smtplib
from email.mime.text import MIMEText

msg = MIMEText('Test email from Docker')
msg['Subject'] = 'Docker Test'
msg['From'] = 'sender@example.com'
msg['To'] = 'test@example.com'

with smtplib.SMTP('localhost', 2525) as server:
    server.send_message(msg)
print('Email sent!')
"
```

### Access Web Interface
```bash
# Open in browser
open http://localhost:3000

# Or curl
curl http://localhost:3000/api/emails/test@example.com
```
