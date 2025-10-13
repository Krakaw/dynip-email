# MCP (Model Context Protocol) Integration

This document describes how to use the MCP server integration for the dynip-email service.

## Overview

The dynip-email service includes an MCP server that allows LLMs to interact with the email system through the Model Context Protocol. This enables AI assistants to read emails, manage webhooks, and perform other email operations.

## Configuration

### Environment Variables

- `MCP_ENABLED`: Enable/disable the MCP server (default: `false`)
- `MCP_PORT`: Port for the MCP server (default: `3001`)

### Example Configuration

```bash
MCP_ENABLED=true
MCP_PORT=3001
```

## MCP Tools

The MCP server provides the following tools for LLM interaction:

### Email Operations

#### `list_emails`
List emails for a specific mailbox.

**Parameters:**
- `mailbox` (string): Email address to list emails for

**Returns:**
- `emails`: Array of email objects
- `count`: Number of emails

#### `read_email`
Get a specific email by ID.

**Parameters:**
- `email_id` (string): Unique email identifier

**Returns:**
- Email object with full content

#### `delete_email`
Delete an email by ID.

**Parameters:**
- `email_id` (string): Unique email identifier

**Returns:**
- Success message

### Webhook Operations

#### `create_webhook`
Create a new webhook for a mailbox.

**Parameters:**
- `mailbox` (string): Email address for the webhook
- `webhook_url` (string): Target URL for webhook calls
- `events` (array): Array of event types (`arrival`, `deletion`, `read`)

**Returns:**
- Created webhook object

#### `list_webhooks`
List webhooks for a mailbox.

**Parameters:**
- `mailbox` (string): Email address to list webhooks for

**Returns:**
- `webhooks`: Array of webhook objects
- `count`: Number of webhooks

#### `delete_webhook`
Delete a webhook by ID.

**Parameters:**
- `webhook_id` (string): Unique webhook identifier

**Returns:**
- Success message

#### `test_webhook`
Test a webhook by sending a test payload.

**Parameters:**
- `webhook_id` (string): Unique webhook identifier

**Returns:**
- `success`: Boolean indicating test result

## MCP Resources

The MCP server provides the following resources:

### `email://{email_id}`
Access email content by ID.

### `webhook://{webhook_id}`
Access webhook configuration by ID.

## Usage Examples

### Connecting to the MCP Server

The MCP server runs on the configured port and accepts MCP protocol connections. LLMs can connect to it using MCP client libraries.

### Example Tool Calls

```json
{
  "tool": "list_emails",
  "arguments": {
    "mailbox": "user@example.com"
  }
}
```

```json
{
  "tool": "create_webhook",
  "arguments": {
    "mailbox": "user@example.com",
    "webhook_url": "https://example.com/webhook",
    "events": ["arrival", "deletion"]
  }
}
```

## Webhook Events

Webhooks can be configured to trigger on the following events:

- `arrival`: When a new email arrives
- `deletion`: When an email is deleted (e.g., due to retention policy)
- `read`: When an email is viewed (optional)

## Webhook Payload Format

When webhooks are triggered, they receive JSON payloads with the following structure:

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

## Security Considerations

- The MCP server should only be accessible to trusted LLM clients
- Consider using authentication/authorization for production deployments
- Webhook URLs should use HTTPS in production
- Monitor webhook delivery and implement retry logic for failed deliveries

## Troubleshooting

### MCP Server Not Starting

1. Check that `MCP_ENABLED=true` is set
2. Verify the port is not already in use
3. Check logs for error messages

### Webhook Delivery Issues

1. Verify webhook URLs are accessible
2. Check network connectivity
3. Review webhook test results
4. Monitor server logs for delivery errors

## Development

To extend the MCP server with additional tools or resources, modify the `src/mcp/mod.rs` file and add new tool/resource handlers.
