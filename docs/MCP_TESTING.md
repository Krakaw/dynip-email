# MCP Server Testing Guide

This guide shows you how to test the MCP (Model Context Protocol) server implementation for dynip-email.

## Quick Start

### 1. Start the MCP Server

```bash
# Enable MCP server and start on port 3001
MCP_ENABLED=true MCP_PORT=3001 cargo run
```

### 2. Run the Test Script

```bash
# Run the automated test script
./test_mcp.sh
```

## Manual Testing

### Server Information

```bash
# Get server info
curl http://localhost:3001/ | jq .
```

Expected response:
```json
{
  "name": "dynip-email-mcp",
  "version": "1.0.0",
  "description": "Email management MCP server for dynip-email",
  "capabilities": {
    "tools": true,
    "resources": true
  }
}
```

### Available Tools

```bash
# List all available tools
curl http://localhost:3001/tools | jq .
```

### Available Resources

```bash
# List all available resources
curl http://localhost:3001/resources | jq .
```

## Tool Testing

### 1. List Emails

```bash
curl -X POST http://localhost:3001/tools/list_emails \
  -H "Content-Type: application/json" \
  -d '{"mailbox": "bob"}' | jq .
```

### 2. Read Specific Email

```bash
curl -X POST http://localhost:3001/tools/read_email \
  -H "Content-Type: application/json" \
  -d '{"email_id": "your-email-id-here"}' | jq .
```

### 3. Create Webhook

```bash
curl -X POST http://localhost:3001/tools/create_webhook \
  -H "Content-Type: application/json" \
  -d '{
    "mailbox": "bob",
    "webhook_url": "http://localhost:3009",
    "events": ["arrival", "deletion"]
  }' | jq .
```

### 4. List Webhooks

```bash
curl -X POST http://localhost:3001/tools/list_webhooks \
  -H "Content-Type: application/json" \
  -d '{"mailbox": "bob"}' | jq .
```

## Resource Testing

### 1. Read Email Resource

```bash
curl http://localhost:3001/resources/email://your-email-id-here | jq .
```

### 2. Read Webhook Resource

```bash
curl http://localhost:3001/resources/webhook://your-webhook-id-here | jq .
```

## Integration Testing

### 1. Test Webhook Flow

1. **Start webhook listener:**
   ```bash
   nc -l 3009
   ```

2. **Create a webhook:**
   ```bash
   curl -X POST http://localhost:3000/api/webhooks \
     -H "Content-Type: application/json" \
     -d '{
       "mailbox_address": "bob",
       "webhook_url": "http://localhost:3009",
       "events": ["arrival"]
     }'
   ```

3. **Send a test email:**
   ```bash
   python3 -c "
   import smtplib
   from email.mime.text import MIMEText
   
   msg = MIMEText('Test webhook email')
   msg['Subject'] = 'Webhook Test'
   msg['From'] = 'test@example.com'
   msg['To'] = 'bob@dyn-ip.me'
   
   with smtplib.SMTP('localhost', 2525) as server:
       server.send_message(msg)
   print('Email sent!')
   "
   ```

4. **Check webhook was triggered** (should see JSON payload in nc output)

### 2. Test MCP Tools with Real Data

1. **List emails via MCP:**
   ```bash
   curl -X POST http://localhost:3001/tools/list_emails \
     -H "Content-Type: application/json" \
     -d '{"mailbox": "bob"}' | jq .
   ```

2. **Get specific email:**
   ```bash
   # Use an email ID from the list above
   curl -X POST http://localhost:3001/tools/read_email \
     -H "Content-Type: application/json" \
     -d '{"email_id": "EMAIL_ID_HERE"}' | jq .
   ```

## Error Testing

### Invalid Tool

```bash
curl -X POST http://localhost:3001/tools/invalid_tool \
  -H "Content-Type: application/json" \
  -d '{}' | jq .
```

Expected: 404 Not Found

### Missing Parameters

```bash
curl -X POST http://localhost:3001/tools/list_emails \
  -H "Content-Type: application/json" \
  -d '{}' | jq .
```

Expected: 400 Bad Request

### Invalid Resource

```bash
curl http://localhost:3001/resources/invalid://test | jq .
```

Expected: 404 Not Found

## Performance Testing

### Concurrent Requests

```bash
# Test multiple concurrent requests
for i in {1..10}; do
  curl -s -X POST http://localhost:3001/tools/list_emails \
    -H "Content-Type: application/json" \
    -d '{"mailbox": "bob"}' &
done
wait
```

### Load Testing

```bash
# Install Apache Bench if not available
# brew install httpd  # macOS
# apt-get install apache2-utils  # Ubuntu

# Test server performance
ab -n 100 -c 10 http://localhost:3001/
```

## Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug MCP_ENABLED=true MCP_PORT=3001 cargo run
```

### Check Server Status

```bash
# Check if MCP server is running
lsof -i :3001

# Check server logs
tail -f /path/to/your/logs
```

### Common Issues

1. **Port already in use:**
   ```bash
   # Kill existing process
   lsof -ti:3001 | xargs kill -9
   ```

2. **MCP server not starting:**
   - Check `MCP_ENABLED=true` is set
   - Verify port 3001 is available
   - Check for compilation errors

3. **Tools not responding:**
   - Verify main email server is running
   - Check database connection
   - Review server logs

## Advanced Testing

### Custom MCP Client

You can create a custom MCP client to test the server:

```python
import requests
import json

class MCPClient:
    def __init__(self, base_url="http://localhost:3001"):
        self.base_url = base_url
    
    def get_server_info(self):
        return requests.get(f"{self.base_url}/").json()
    
    def list_tools(self):
        return requests.get(f"{self.base_url}/tools").json()
    
    def call_tool(self, tool_name, params):
        return requests.post(
            f"{self.base_url}/tools/{tool_name}",
            json=params
        ).json()
    
    def list_resources(self):
        return requests.get(f"{self.base_url}/resources").json()
    
    def read_resource(self, resource_id):
        return requests.get(f"{self.base_url}/resources/{resource_id}").json()

# Usage
client = MCPClient()
print("Server Info:", client.get_server_info())
print("Tools:", client.list_tools())
print("Emails:", client.call_tool("list_emails", {"mailbox": "bob"}))
```

This comprehensive testing approach ensures your MCP server is working correctly and can handle real-world usage scenarios.
