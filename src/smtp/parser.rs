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
    let subject = message
        .subject()
        .unwrap_or("(No Subject)")
        .to_string();
    
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

