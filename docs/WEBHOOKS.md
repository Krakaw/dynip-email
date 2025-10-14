# Webhook Configuration Guide

This document explains how to configure and use webhooks with the dynip-email service.

## Overview

Webhooks allow you to receive real-time notifications when email events occur. You can configure webhooks on a per-mailbox basis and choose which events should trigger notifications.

## Supported Events

- **Email Arrival**: Triggered when a new email is received
- **Email Deletion**: Triggered when an email is deleted (e.g., due to retention policy)

## Configuration

### Via Web Interface

1. Navigate to the email interface
2. Enter an email address to load the mailbox
3. Click the "Webhooks" tab
4. Click "Add Webhook" to create a new webhook
5. Configure the webhook URL and select events
6. Test the webhook to verify it works

### Via API

#### Create Webhook

```bash
curl -X POST http://localhost:3000/api/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "mailbox_address": "user@example.com",
    "webhook_url": "https://example.com/webhook",
    "events": ["arrival", "deletion"]
  }'
```

**Note**: The webhook URL must include the protocol scheme (`http://` or `https://`). For local testing, use `http://localhost:PORT`.

#### List Webhooks

```bash
curl http://localhost:3000/api/webhooks/user@example.com
```

#### Update Webhook

```bash
curl -X PUT http://localhost:3000/api/webhook/{webhook_id} \
  -H "Content-Type: application/json" \
  -d '{
    "webhook_url": "https://new-url.com/webhook",
    "events": ["arrival"],
    "enabled": true
  }'
```

#### Delete Webhook

```bash
curl -X DELETE http://localhost:3000/api/webhook/{webhook_id}
```

#### Test Webhook

```bash
curl -X POST http://localhost:3000/api/webhook/{webhook_id}/test
```

## Webhook Payload Format

When webhooks are triggered, they receive HTTP POST requests with JSON payloads:

### Email Arrival Event

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
    "attachments": 2
  }
}
```

### Email Deletion Event

```json
{
  "event": "deletion",
  "mailbox": "user@example.com",
  "webhook_id": "webhook-uuid",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### Test Event

```json
{
  "event": "test",
  "mailbox": "user@example.com",
  "webhook_id": "webhook-uuid",
  "timestamp": "2024-01-01T00:00:00Z",
  "message": "This is a test webhook payload"
}
```

## Webhook Implementation

### Basic Webhook Handler

Here's an example webhook handler in Python:

```python
from flask import Flask, request, jsonify
import json

app = Flask(__name__)

@app.route('/webhook', methods=['POST'])
def handle_webhook():
    data = request.get_json()
    
    if data['event'] == 'arrival':
        print(f"New email received: {data['email']['subject']}")
        # Process new email
    elif data['event'] == 'deletion':
        print(f"Email deleted for {data['mailbox']}")
        # Process email deletion
    elif data['event'] == 'test':
        print("Webhook test successful")
        # Return success response
    
    return jsonify({"status": "success"})

if __name__ == '__main__':
    app.run(port=5000)
```

### Node.js Example

```javascript
const express = require('express');
const app = express();

app.use(express.json());

app.post('/webhook', (req, res) => {
    const { event, mailbox, email } = req.body;
    
    switch (event) {
        case 'arrival':
            console.log(`New email: ${email.subject}`);
            break;
        case 'deletion':
            console.log(`Email deleted for ${mailbox}`);
            break;
        case 'test':
            console.log('Webhook test successful');
            break;
    }
    
    res.json({ status: 'success' });
});

app.listen(5000, () => {
    console.log('Webhook server running on port 5000');
});
```

## Retry Logic

The webhook system includes automatic retry logic:

- **Retry Attempts**: 3 attempts maximum
- **Retry Delay**: Exponential backoff (2^attempt seconds)
- **Timeout**: 30 seconds per request
- **Failure Handling**: Logs errors but doesn't block email processing

## Security Best Practices

1. **Use HTTPS**: Always use HTTPS URLs for webhooks in production
2. **Validate Payloads**: Verify webhook payloads using signatures or tokens
3. **Rate Limiting**: Implement rate limiting on your webhook endpoints
4. **Authentication**: Use authentication tokens if needed
5. **Monitoring**: Monitor webhook delivery success rates

### Example with Signature Validation

```python
import hmac
import hashlib

def verify_webhook_signature(payload, signature, secret):
    expected_signature = hmac.new(
        secret.encode(),
        payload.encode(),
        hashlib.sha256
    ).hexdigest()
    
    return hmac.compare_digest(signature, expected_signature)

@app.route('/webhook', methods=['POST'])
def handle_webhook():
    signature = request.headers.get('X-Webhook-Signature')
    if not verify_webhook_signature(request.data, signature, 'your-secret'):
        return 'Unauthorized', 401
    
    # Process webhook...
```

## Troubleshooting

### Webhook Not Receiving Events

1. **Check Configuration**: Verify webhook URL and events are configured correctly
2. **Test Webhook**: Use the test button in the web interface
3. **Check Logs**: Review server logs for webhook delivery errors
4. **Network Issues**: Ensure the webhook URL is accessible from the server

### Common Issues

- **Timeout Errors**: Increase timeout or optimize webhook handler performance
- **Authentication Failures**: Check webhook URL authentication requirements
- **Payload Issues**: Verify webhook handler can parse JSON payloads
- **Rate Limiting**: Implement proper rate limiting on webhook endpoints

### Debugging

Enable debug logging to see webhook delivery details:

```bash
RUST_LOG=debug cargo run
```

## Debugging Webhook Issues

### Common Failure Scenarios

#### 1. Connection Errors
- **Symptom**: "Connection error: ... - Check if the webhook URL is reachable and the server is running"
- **Cause**: No server listening on the webhook URL
- **Solution**: Start a webhook test server or ensure your webhook endpoint is running

#### 2. Timeout Errors
- **Symptom**: "Timeout error: ..."
- **Cause**: Webhook server takes too long to respond (>10 seconds)
- **Solution**: Optimize your webhook endpoint or increase timeout in code

#### 3. HTTP Status Errors
- **Symptom**: "Webhook failed with status 4xx/5xx"
- **Cause**: Webhook server returns error status
- **Solution**: Check your webhook endpoint implementation

#### 4. URL Format Errors
- **Symptom**: "Request error: ... - Check the webhook URL format"
- **Cause**: Invalid URL format
- **Solution**: Ensure URL includes protocol (http:// or https://)

### Debugging Tools

#### Enhanced Logging

The webhook system includes detailed logging with emojis for easy identification:

- 🚀 **Webhook Start**: When a webhook is being sent
- 📦 **Payload**: The JSON payload being sent
- 🔄 **Retry**: When a webhook is being retried
- 📡 **Response**: HTTP response received
- ✅ **Success**: Webhook sent successfully
- ❌ **Failure**: Webhook failed with details
- 💥 **Final Failure**: Webhook failed after all retries

#### Test Webhook Server

Use the included Python webhook server to debug webhook issues:

```bash
# Start the webhook test server
python3 scripts/webhook_server.py

# The server will start on port 3009 and display all incoming webhook requests
```

The webhook server will:
- Display all incoming webhook requests
- Show headers and payload
- Return success responses
- Help identify payload format issues

#### Manual Testing

Test webhooks manually using curl:

```bash
# Test with a simple payload
curl -X POST http://localhost:3009 \
  -H "Content-Type: application/json" \
  -d '{"event":"test","mailbox":"test","message":"Hello World"}'
```

### Debugging Steps

1. **Check Webhook Configuration**: Verify webhook URL is correct and accessible
2. **Test Webhook Endpoint**: Start the webhook server and configure webhook to point to it
3. **Analyze Logs**: Look for log patterns showing success or failure
4. **Common Issues**: Check for connection errors, timeouts, HTTP status errors, or URL format issues

### Environment Variables for Debugging

```bash
# Enable debug logging
export RUST_LOG=debug

# Enable webhook debug logging specifically
export RUST_LOG=dynip_email::webhooks=debug

# Run with debug logging
RUST_LOG=debug cargo run
```

### Troubleshooting Checklist

- [ ] Webhook URL is correct and accessible
- [ ] URL includes proper protocol (http:// or https://)
- [ ] Webhook endpoint is running and responding
- [ ] Network connectivity is working
- [ ] Webhook endpoint returns 2xx status codes
- [ ] Payload format is correct
- [ ] No firewall blocking the connection
- [ ] SSL certificates are valid (for HTTPS)
- [ ] Webhook endpoint handles POST requests
- [ ] Content-Type is application/json

## Monitoring

Monitor webhook delivery success rates and implement alerting for failed deliveries. Consider using webhook delivery services for production deployments.
