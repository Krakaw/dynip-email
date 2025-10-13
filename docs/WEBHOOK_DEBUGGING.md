# Webhook Debugging Guide

## Overview

This guide helps you debug webhook failures in the dynip-email service. Webhooks can fail for various reasons, and this guide provides tools and techniques to identify and resolve issues.

## Common Webhook Failure Scenarios

### 1. Connection Errors
- **Symptom**: "Connection error: ... - Check if the webhook URL is reachable and the server is running"
- **Cause**: No server listening on the webhook URL
- **Solution**: Start a webhook test server or ensure your webhook endpoint is running

### 2. Timeout Errors
- **Symptom**: "Timeout error: ..."
- **Cause**: Webhook server takes too long to respond (>10 seconds)
- **Solution**: Optimize your webhook endpoint or increase timeout in code

### 3. HTTP Status Errors
- **Symptom**: "Webhook failed with status 4xx/5xx"
- **Cause**: Webhook server returns error status
- **Solution**: Check your webhook endpoint implementation

### 4. URL Format Errors
- **Symptom**: "Request error: ... - Check the webhook URL format"
- **Cause**: Invalid URL format
- **Solution**: Ensure URL includes protocol (http:// or https://)

## Debugging Tools

### 1. Enhanced Logging

The webhook system now includes detailed logging with emojis for easy identification:

- 🚀 **Webhook Start**: When a webhook is being sent
- 📦 **Payload**: The JSON payload being sent
- 🔄 **Retry**: When a webhook is being retried
- 📡 **Response**: HTTP response received
- ✅ **Success**: Webhook sent successfully
- ❌ **Failure**: Webhook failed with details
- 💥 **Final Failure**: Webhook failed after all retries

### 2. Test Webhook Server

Use the included Python test server to debug webhook issues:

```bash
# Start the test server
python3 test_webhook_server.py

# In another terminal, test the webhook
./test_webhook.sh http://localhost:3009
```

The test server will:
- Display all incoming webhook requests
- Show headers and payload
- Return success responses
- Help identify payload format issues

### 3. Manual Webhook Testing

Test webhooks manually using curl:

```bash
# Test with a simple payload
curl -X POST http://localhost:3009 \
  -H "Content-Type: application/json" \
  -d '{"event":"test","mailbox":"test","message":"Hello World"}'
```

### 4. Webhook Test Script

Use the included test script:

```bash
# Test default localhost:3009
./test_webhook.sh

# Test custom URL
./test_webhook.sh https://your-webhook-endpoint.com/webhook
```

## Debugging Steps

### Step 1: Check Webhook Configuration

1. Verify webhook URL is correct and accessible
2. Ensure URL includes protocol (http:// or https://)
3. Check if webhook endpoint is running and responding

### Step 2: Test Webhook Endpoint

1. Start the test server: `python3 test_webhook_server.py`
2. Configure webhook to point to `http://localhost:3009`
3. Send a test email to trigger webhook
4. Check test server output for received payload

### Step 3: Analyze Logs

Look for these log patterns:

```
🚀 Sending webhook abc123 to URL: http://localhost:3009
📦 Webhook payload: {"event":"arrival","mailbox":"test",...}
🔄 Webhook abc123 attempt 1/3
📡 Webhook abc123 received response: 200 OK
✅ Webhook abc123 sent successfully to http://localhost:3009 (status: 200)
```

Or for failures:

```
❌ Webhook abc123 attempt 1 failed: Connection error: ... - Check if the webhook URL is reachable and the server is running
⏳ Retrying webhook abc123 in 1s
💥 Webhook abc123 failed after 3 attempts. Last error: Connection error: ...
```

### Step 4: Common Issues and Solutions

#### Issue: "Connection error"
**Solution**: 
- Start a webhook test server
- Check if webhook URL is correct
- Verify network connectivity

#### Issue: "Timeout error"
**Solution**:
- Optimize your webhook endpoint
- Check server performance
- Consider increasing timeout (modify code)

#### Issue: "HTTP 404/500"
**Solution**:
- Check webhook endpoint implementation
- Verify endpoint URL path
- Check server logs for errors

#### Issue: "Request error"
**Solution**:
- Verify URL format includes protocol
- Check for typos in webhook URL
- Ensure URL is properly encoded

## Webhook Payload Format

The webhook system sends JSON payloads with this structure:

```json
{
  "event": "arrival|deletion|read",
  "mailbox": "mailbox_name",
  "webhook_id": "webhook-uuid",
  "timestamp": "2025-01-13T14:30:00Z",
  "email": {
    "id": "email-uuid",
    "to": "recipient@example.com",
    "from": "sender@example.com",
    "subject": "Email Subject",
    "body": "Email body content",
    "timestamp": "2025-01-13T14:30:00Z",
    "attachments": 0
  }
}
```

## Environment Variables for Debugging

Set these environment variables for enhanced debugging:

```bash
# Enable debug logging
export RUST_LOG=debug

# Enable webhook debug logging specifically
export RUST_LOG=dynip_email::webhooks=debug

# Run with debug logging
RUST_LOG=debug cargo run
```

## Production Webhook Endpoints

For production webhook endpoints, ensure:

1. **HTTPS**: Use HTTPS for security
2. **Authentication**: Implement proper authentication
3. **Rate Limiting**: Handle rate limiting gracefully
4. **Idempotency**: Handle duplicate webhook deliveries
5. **Error Handling**: Return appropriate HTTP status codes
6. **Logging**: Log webhook deliveries for debugging

## Monitoring Webhook Health

Monitor webhook health by:

1. **Log Analysis**: Parse logs for webhook success/failure rates
2. **Metrics**: Track webhook delivery metrics
3. **Alerts**: Set up alerts for webhook failures
4. **Testing**: Regular webhook endpoint testing

## Troubleshooting Checklist

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

## Getting Help

If webhook issues persist:

1. Check the enhanced logs for specific error messages
2. Use the test server to verify webhook format
3. Test with curl to isolate the issue
4. Check webhook endpoint server logs
5. Verify network connectivity and DNS resolution
