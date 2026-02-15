-- Migration script to populate FTS5 table with existing emails
-- Run this after upgrading to a version with FTS5 support

-- Populate FTS table with existing emails
INSERT INTO emails_fts(rowid, id, to_address, from_address, subject, body)
SELECT rowid, id, to_address, from_address, subject, body
FROM emails
WHERE rowid NOT IN (SELECT rowid FROM emails_fts);
