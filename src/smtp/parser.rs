use anyhow::{anyhow, Result};
use mail_parser::MessageParser;

use crate::storage::models::Email;

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
    
    // Store raw email
    let raw = String::from_utf8_lossy(raw_email).to_string();
    
    Ok(Email::new(
        recipient,
        from,
        subject,
        body,
        Some(raw),
    ))
}

