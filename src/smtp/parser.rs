use anyhow::{anyhow, Result};
use mail_parser::{MessageParser, MimeHeaders};

use crate::storage::models::{Attachment, Email};

/// Parse raw email data into an Email struct
pub fn parse_email(raw_email: &[u8], fallback_recipient: &str) -> Result<Email> {
    let parser = MessageParser::default();
    let message = parser
        .parse(raw_email)
        .ok_or_else(|| anyhow!("Failed to parse email"))?;

    // Extract recipient (To address)
    let recipient = message
        .to()
        .and_then(|addrs| addrs.first())
        .and_then(|addr| addr.address())
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback_recipient.to_string());

    // Extract from address
    let from = message
        .from()
        .and_then(|addrs| addrs.first())
        .and_then(|addr| addr.address())
        .unwrap_or("unknown@unknown.com")
        .to_string();

    // Extract subject
    let subject = message.subject().unwrap_or("(No Subject)").to_string();

    // Extract body (prefer HTML, fallback to text)
    let body = if let Some(html) = message.body_html(0) {
        html.to_string()
    } else if let Some(text) = message.body_text(0) {
        text.to_string()
    } else {
        "(No body)".to_string()
    };

    // Extract attachments
    let mut attachments = Vec::new();
    for attachment in message.attachments() {
        let body = attachment.contents();

        let content_type = attachment
            .content_type()
            .map(|ct| ct.ctype().to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let filename = attachment
            .attachment_name()
            .unwrap_or("attachment")
            .to_string();

        // Base64 encode the content for storage
        let content = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, body);

        attachments.push(Attachment {
            filename,
            content_type,
            size: body.len(),
            content,
        });
    }

    // Store raw email
    let raw = String::from_utf8_lossy(raw_email).to_string();

    Ok(Email::new(
        recipient,
        from,
        subject,
        body,
        Some(raw),
        attachments,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_simple_email() -> Vec<u8> {
        b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Test Subject\r\n\r\nThis is a test email body.".to_vec()
    }

    fn create_email_with_attachment() -> Vec<u8> {
        b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Test with Attachment\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"boundary123\"\r\n\r\n--boundary123\r\nContent-Type: text/plain\r\n\r\nThis is the email body.\r\n\r\n--boundary123\r\nContent-Type: text/plain\r\nContent-Disposition: attachment; filename=\"test.txt\"\r\n\r\nThis is attachment content.\r\n\r\n--boundary123--".to_vec()
    }

    fn create_html_email() -> Vec<u8> {
        b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: HTML Email\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Hello World</h1><p>This is an HTML email.</p></body></html>".to_vec()
    }

    fn create_email_without_subject() -> Vec<u8> {
        b"From: sender@example.com\r\nTo: recipient@example.com\r\n\r\nThis email has no subject."
            .to_vec()
    }

    fn create_email_without_from() -> Vec<u8> {
        b"To: recipient@example.com\r\nSubject: No From Header\r\n\r\nThis email has no from header.".to_vec()
    }

    #[test]
    fn test_parse_simple_email() {
        let raw_email = create_simple_email();
        let email = parse_email(&raw_email, "fallback@example.com").unwrap();

        assert_eq!(email.to, "recipient@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "Test Subject");
        assert!(email.body.contains("This is a test email body."));
        assert!(email.attachments.is_empty());
        assert!(email.raw.is_some());
    }

    #[test]
    fn test_parse_email_with_fallback_recipient() {
        let raw_email =
            b"From: sender@example.com\r\nSubject: Test Subject\r\n\r\nThis is a test email body."
                .to_vec();
        let email = parse_email(&raw_email, "fallback@example.com").unwrap();

        assert_eq!(email.to, "fallback@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "Test Subject");
        assert!(email.body.contains("This is a test email body."));
    }

    #[test]
    fn test_parse_email_without_subject() {
        let raw_email = create_email_without_subject();
        let email = parse_email(&raw_email, "recipient@example.com").unwrap();

        assert_eq!(email.to, "recipient@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "(No Subject)");
        assert!(email.body.contains("This email has no subject."));
    }

    #[test]
    fn test_parse_email_without_from() {
        let raw_email = create_email_without_from();
        let email = parse_email(&raw_email, "recipient@example.com").unwrap();

        assert_eq!(email.to, "recipient@example.com");
        assert_eq!(email.from, "unknown@unknown.com");
        assert_eq!(email.subject, "No From Header");
        assert!(email.body.contains("This email has no from header."));
    }

    #[test]
    fn test_parse_html_email() {
        let raw_email = create_html_email();
        let email = parse_email(&raw_email, "recipient@example.com").unwrap();

        assert_eq!(email.to, "recipient@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "HTML Email");
        assert!(email.body.contains("<html>"));
        assert!(email.body.contains("<h1>Hello World</h1>"));
    }

    #[test]
    fn test_parse_email_with_attachment() {
        let raw_email = create_email_with_attachment();
        let email = parse_email(&raw_email, "recipient@example.com").unwrap();

        assert_eq!(email.to, "recipient@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "Test with Attachment");
        assert!(!email.attachments.is_empty());

        // Check attachment details
        let attachment = &email.attachments[0];
        assert_eq!(attachment.filename, "test.txt");
        assert!(attachment.content_type.contains("text"));
        assert!(attachment.content.len() > 0);
    }

    #[test]
    fn test_parse_invalid_email() {
        let invalid_email = b"Invalid email content without proper headers".to_vec();
        let result = parse_email(&invalid_email, "fallback@example.com");

        // The parser might still succeed with fallback values
        // Let's just check that we get some result
        match result {
            Ok(email) => {
                assert_eq!(email.to, "fallback@example.com");
                assert_eq!(email.from, "unknown@unknown.com");
            }
            Err(_) => {
                // This is also acceptable - parser failed as expected
            }
        }
    }

    #[test]
    fn test_parse_empty_email() {
        let empty_email = b"".to_vec();
        let result = parse_email(&empty_email, "fallback@example.com");

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_email_with_multiple_recipients() {
        let raw_email = b"From: sender@example.com\r\nTo: recipient1@example.com, recipient2@example.com\r\nSubject: Multiple Recipients\r\n\r\nThis email has multiple recipients.".to_vec();
        let email = parse_email(&raw_email, "fallback@example.com").unwrap();

        // Should use the first recipient
        assert_eq!(email.to, "recipient1@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "Multiple Recipients");
    }

    #[test]
    fn test_parse_email_with_complex_headers() {
        let raw_email = b"From: \"John Doe\" <john.doe@example.com>\r\nTo: \"Jane Smith\" <jane.smith@example.com>\r\nSubject: Complex Headers\r\nDate: Mon, 1 Jan 2024 12:00:00 +0000\r\n\r\nThis email has complex headers with display names.".to_vec();
        let email = parse_email(&raw_email, "fallback@example.com").unwrap();

        assert_eq!(email.to, "jane.smith@example.com");
        assert_eq!(email.from, "john.doe@example.com");
        assert_eq!(email.subject, "Complex Headers");
    }

    #[test]
    fn test_parse_email_with_unicode_content() {
        let raw_email = "From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Unicode Test\r\n\r\nHello 世界! This email contains Unicode characters.".as_bytes().to_vec();
        let email = parse_email(&raw_email, "recipient@example.com").unwrap();

        assert_eq!(email.to, "recipient@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "Unicode Test");
        assert!(email.body.contains("Hello 世界!"));
    }

    #[test]
    fn test_parse_email_with_base64_attachment() {
        let raw_email = b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Base64 Attachment\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"boundary123\"\r\n\r\n--boundary123\r\nContent-Type: text/plain\r\n\r\nThis is the email body.\r\n\r\n--boundary123\r\nContent-Type: text/plain\r\nContent-Disposition: attachment; filename=\"test.txt\"\r\nContent-Transfer-Encoding: base64\r\n\r\nVGVzdCBhdHRhY2htZW50IGNvbnRlbnQ=\r\n\r\n--boundary123--".to_vec();
        let email = parse_email(&raw_email, "recipient@example.com").unwrap();

        assert_eq!(email.to, "recipient@example.com");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "Base64 Attachment");
        assert!(!email.attachments.is_empty());

        let attachment = &email.attachments[0];
        assert_eq!(attachment.filename, "test.txt");
        assert!(attachment.content_type.contains("text"));
        // The content should be base64 encoded
        assert!(attachment.content.len() > 0);
    }
}
