# Full-Text Search Feature

## Overview

This feature adds SQLite FTS5 (Full-Text Search 5) support to dynip-email, allowing users to search through email content quickly and efficiently.

## Features

- **Full-text indexing** of email subject, body, from, and to fields
- **Real-time synchronization** via database triggers
- **Search highlighting** in results with `<mark>` tags
- **Mailbox filtering** to search within a specific mailbox
- **Password protection** support for locked mailboxes
- **Ranked results** by relevance

## API

### Search Endpoint

```
GET /api/search?q=<query>[&mailbox=<address>][&limit=<n>][&password=<pass>]
```

**Parameters:**
- `q` (required): Search query string (supports FTS5 syntax)
- `mailbox` (optional): Filter results to specific mailbox
- `limit` (optional): Maximum number of results (default: 50)
- `password` (optional): Password for protected mailboxes

**Example:**
```bash
curl "https://mail.dyn-ip.me/api/search?q=invoice&mailbox=alice@example.com&limit=10"
```

**Response:**
```json
{
  "results": [
    {
      "id": "abc123",
      "to": "alice@example.com",
      "from": "billing@example.com",
      "subject": "Monthly Invoice",
      "snippet": "Your <mark>invoice</mark> for January is ready...",
      "timestamp": "2026-02-15T10:30:00Z",
      "rank": -1.23
    }
  ]
}
```

## Search Syntax

The search supports FTS5 query syntax:

- **AND**: `word1 AND word2`
- **OR**: `word1 OR word2`
- **NOT**: `word1 NOT word2`
- **Phrases**: `"exact phrase"`
- **Prefix**: `word*`
- **Column-specific**: `subject:invoice` or `from:billing`

**Examples:**
- `invoice payment` - Find emails with both words
- `"order confirmation"` - Exact phrase match
- `urgent AND NOT spam` - Urgent emails excluding spam
- `subject:meeting` - Search only in subject line
- `from:@company.com` - Search emails from a domain

## UI

The search interface is accessible via the "Search" tab in the web UI:

1. Click the "Search" tab
2. Enter your search query
3. Click "Search" or press Enter
4. Results show with highlighted matches
5. Click a result to view the full email

## Database Schema

### FTS5 Virtual Table

```sql
CREATE VIRTUAL TABLE emails_fts USING fts5(
    id UNINDEXED,
    to_address,
    from_address,
    subject,
    body,
    content='emails',
    content_rowid='rowid'
);
```

### Triggers

The FTS table is automatically synchronized with the main `emails` table via triggers:
- `emails_ai`: After INSERT
- `emails_ad`: After DELETE
- `emails_au`: After UPDATE

## Migration

For existing databases, run the migration script to populate the FTS table:

```bash
sqlite3 /path/to/database.db < scripts/migrate_fts.sql
```

The migration is idempotent and only inserts missing entries.

## Performance

- FTS5 provides fast full-text search even with thousands of emails
- Index is updated automatically on email arrival/deletion
- Search queries typically complete in <10ms
- Result ranking by relevance (BM25 algorithm)

## Future Enhancements

- Date range filtering
- Attachment content indexing
- Advanced query builder UI
- Search result pagination
- Search history/suggestions
