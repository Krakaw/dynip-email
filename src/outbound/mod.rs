use anyhow::{Context, Result};
use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::Deserialize;
use std::sync::Arc;

use crate::config::Config;
use crate::dkim::DkimSigner;

/// Configuration for SMTP relay transport
#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Outbound email mailer
pub struct OutboundMailer {
    dkim_signer: Option<Arc<DkimSigner>>,
    relay: Option<RelayConfig>,
    from_domain: String,
}

/// Request to send an email
#[derive(Debug, Deserialize)]
pub struct SendEmailRequest {
    pub to: String,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub from_name: Option<String>,
    pub from_address: Option<String>,
}

impl OutboundMailer {
    pub fn new(config: &Config, dkim_signer: Option<Arc<DkimSigner>>) -> Result<Self> {
        let relay = match &config.smtp_relay_host {
            Some(host) => Some(RelayConfig {
                host: host.clone(),
                port: config.smtp_relay_port.unwrap_or(587),
                username: config.smtp_relay_username.clone(),
                password: config.smtp_relay_password.clone(),
            }),
            None => None,
        };

        let from_domain = config
            .dkim_domain
            .clone()
            .unwrap_or_else(|| config.domain_name.clone());

        Ok(Self {
            dkim_signer,
            relay,
            from_domain,
        })
    }

    pub fn from_domain(&self) -> &str {
        &self.from_domain
    }

    /// Send an email, returning the message ID
    pub async fn send_email(&self, request: &SendEmailRequest) -> Result<String> {
        let from_local = request
            .from_address
            .as_deref()
            .unwrap_or("noreply");
        let from_email = format!("{}@{}", from_local, self.from_domain);

        let from_mailbox: Mailbox = if let Some(ref name) = request.from_name {
            format!("{} <{}>", name, from_email)
                .parse()
                .context("Invalid from address")?
        } else {
            from_email
                .parse()
                .context("Invalid from address")?
        };

        let to_mailbox: Mailbox = request
            .to
            .parse()
            .context("Invalid recipient address")?;

        // Build the message
        let builder = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(&request.subject);

        let message = if let Some(ref html) = request.body_html {
            builder.multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(request.body_text.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html.clone()),
                    ),
            )?
        } else {
            builder.body(request.body_text.clone())?
        };

        let message_id = message
            .headers()
            .get_raw("Message-ID")
            .map(|v| v.to_string())
            .unwrap_or_default();

        // Get raw message bytes for DKIM signing
        let raw_message = message.formatted();

        // Sign with DKIM if available
        let final_message = if let Some(ref signer) = self.dkim_signer {
            signer.sign(&raw_message).context("DKIM signing failed")?
        } else {
            raw_message
        };

        // Send via relay or direct MX
        if let Some(ref relay) = self.relay {
            self.send_via_relay(relay, &final_message).await?;
        } else {
            self.send_direct_mx(&request.to, &final_message).await?;
        }

        Ok(message_id)
    }

    async fn send_via_relay(&self, relay: &RelayConfig, message: &[u8]) -> Result<()> {
        let mut transport_builder =
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&relay.host)
                .context("Failed to create SMTP relay transport")?
                .port(relay.port);

        if let (Some(ref user), Some(ref pass)) = (&relay.username, &relay.password) {
            transport_builder =
                transport_builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }

        let transport = transport_builder.build();

        let envelope = lettre::address::Envelope::new(
            self.extract_sender_from_message(message),
            vec![self.extract_recipient_from_message(message)]
                .into_iter()
                .flatten()
                .collect(),
        )
        .context("Failed to create envelope")?;

        transport
            .send_raw(&envelope, message)
            .await
            .context("Failed to send email via relay")?;

        Ok(())
    }

    async fn send_direct_mx(&self, to: &str, message: &[u8]) -> Result<()> {
        let domain = to
            .split('@')
            .nth(1)
            .context("Invalid recipient address: no domain")?;

        // Look up MX records
        let resolver = hickory_resolver::TokioAsyncResolver::tokio_from_system_conf()
            .context("Failed to create DNS resolver")?;

        let mx_response = resolver
            .mx_lookup(domain)
            .await
            .context("MX lookup failed")?;

        let mx_host = mx_response
            .iter()
            .min_by_key(|mx| mx.preference())
            .map(|mx| mx.exchange().to_string())
            .context("No MX records found")?;

        // Strip trailing dot from DNS name
        let mx_host = mx_host.trim_end_matches('.');

        let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(mx_host)
            .port(25)
            .build();

        let envelope = lettre::address::Envelope::new(
            self.extract_sender_from_message(message),
            vec![to.parse().ok()]
                .into_iter()
                .flatten()
                .collect(),
        )
        .context("Failed to create envelope")?;

        transport
            .send_raw(&envelope, message)
            .await
            .context("Failed to send email via direct MX")?;

        Ok(())
    }

    fn extract_sender_from_message(&self, _message: &[u8]) -> Option<lettre::Address> {
        // Parse From header from raw message
        let msg_str = String::from_utf8_lossy(_message);
        for line in msg_str.lines() {
            if let Some(rest) = line.strip_prefix("From:") {
                let rest = rest.trim();
                // Handle "Name <email>" or bare "email" format
                if let Some(start) = rest.find('<') {
                    if let Some(end) = rest.find('>') {
                        return rest[start + 1..end].parse().ok();
                    }
                }
                return rest.parse().ok();
            }
        }
        None
    }

    fn extract_recipient_from_message(&self, _message: &[u8]) -> Option<lettre::Address> {
        let msg_str = String::from_utf8_lossy(_message);
        for line in msg_str.lines() {
            if let Some(rest) = line.strip_prefix("To:") {
                let rest = rest.trim();
                if let Some(start) = rest.find('<') {
                    if let Some(end) = rest.find('>') {
                        return rest[start + 1..end].parse().ok();
                    }
                }
                return rest.parse().ok();
            }
        }
        None
    }
}
