//! IMAP server implementation for retrieving emails
//!
//! This module provides a minimal IMAP server that supports:
//! - LOGIN authentication using mailbox address and password
//! - LIST/LSUB for listing mailboxes
//! - SELECT for selecting a mailbox
//! - FETCH for retrieving emails
//! - SEARCH for searching emails
//! - LOGOUT for disconnecting

use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use crate::storage::StorageBackend;

/// IMAP server that handles client connections
pub struct ImapServer {
    storage: Arc<dyn StorageBackend>,
    domain_name: String,
}

impl ImapServer {
    /// Create a new IMAP server instance
    pub fn new(storage: Arc<dyn StorageBackend>, domain_name: String) -> Self {
        Self {
            storage,
            domain_name,
        }
    }

    /// Start the IMAP server on the specified port
    pub async fn start(&self, port: u16) -> Result<()> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        info!("📬 IMAP server listening on port {}", port);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("IMAP connection from {}", addr);
                    let storage = self.storage.clone();
                    let domain_name = self.domain_name.clone();

                    tokio::spawn(async move {
                        if let Err(e) = ImapConnection::new(stream, storage, domain_name)
                            .handle()
                            .await
                        {
                            error!("IMAP connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept IMAP connection: {}", e);
                }
            }
        }
    }
}

/// Represents the state of an IMAP connection
#[derive(Debug, Clone, PartialEq)]
enum ImapState {
    NotAuthenticated,
    Authenticated,
    Selected(String), // Contains the selected mailbox name
}

/// Handles a single IMAP client connection
struct ImapConnection {
    stream: BufReader<TcpStream>,
    storage: Arc<dyn StorageBackend>,
    domain_name: String,
    state: ImapState,
    authenticated_user: Option<String>,
}

impl ImapConnection {
    fn new(stream: TcpStream, storage: Arc<dyn StorageBackend>, domain_name: String) -> Self {
        Self {
            stream: BufReader::new(stream),
            storage,
            domain_name,
            state: ImapState::NotAuthenticated,
            authenticated_user: None,
        }
    }

    async fn handle(&mut self) -> Result<()> {
        // Send greeting
        self.send_line("* OK IMAP4rev1 Service Ready").await?;

        let mut line = String::new();
        loop {
            line.clear();
            match self.stream.read_line(&mut line).await {
                Ok(0) => {
                    debug!("IMAP client disconnected");
                    break;
                }
                Ok(_) => {
                    let line = line.trim();
                    debug!("IMAP received: {}", line);

                    if let Err(e) = self.process_command(line).await {
                        error!("IMAP command error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("IMAP read error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn send_line(&mut self, line: &str) -> Result<()> {
        debug!("IMAP sending: {}", line);
        self.stream
            .get_mut()
            .write_all(format!("{}\r\n", line).as_bytes())
            .await?;
        Ok(())
    }

    async fn process_command(&mut self, line: &str) -> Result<()> {
        // Parse tag and command
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.is_empty() {
            return Ok(());
        }

        let tag = parts[0];
        let command = parts.get(1).map(|s| s.to_uppercase()).unwrap_or_default();
        let args = parts.get(2).copied().unwrap_or("");

        match command.as_str() {
            "CAPABILITY" => self.cmd_capability(tag).await,
            "NOOP" => self.cmd_noop(tag).await,
            "LOGOUT" => self.cmd_logout(tag).await,
            "LOGIN" => self.cmd_login(tag, args).await,
            "AUTHENTICATE" => self.cmd_authenticate(tag, args).await,
            "LIST" => self.cmd_list(tag, args).await,
            "LSUB" => self.cmd_lsub(tag, args).await,
            "SELECT" => self.cmd_select(tag, args).await,
            "EXAMINE" => self.cmd_examine(tag, args).await,
            "FETCH" => self.cmd_fetch(tag, args).await,
            "SEARCH" => self.cmd_search(tag, args).await,
            "CLOSE" => self.cmd_close(tag).await,
            "UID" => self.cmd_uid(tag, args).await,
            _ => {
                self.send_line(&format!("{} BAD Unknown command", tag))
                    .await
            }
        }
    }

    async fn cmd_capability(&mut self, tag: &str) -> Result<()> {
        self.send_line("* CAPABILITY IMAP4rev1 AUTH=PLAIN LOGIN")
            .await?;
        self.send_line(&format!("{} OK CAPABILITY completed", tag))
            .await
    }

    async fn cmd_noop(&mut self, tag: &str) -> Result<()> {
        self.send_line(&format!("{} OK NOOP completed", tag)).await
    }

    async fn cmd_logout(&mut self, tag: &str) -> Result<()> {
        self.send_line("* BYE IMAP4rev1 Server logging out").await?;
        self.send_line(&format!("{} OK LOGOUT completed", tag))
            .await?;
        // Signal to close the connection
        Err(anyhow::anyhow!("Client logged out"))
    }

    async fn cmd_authenticate(&mut self, tag: &str, args: &str) -> Result<()> {
        let mechanism = args.trim().to_uppercase();
        
        if mechanism != "PLAIN" {
            return self
                .send_line(&format!("{} NO Unsupported authentication mechanism", tag))
                .await;
        }

        // Send continuation request
        self.send_line("+").await?;

        // Read the base64-encoded credentials
        let mut line = String::new();
        match self.stream.read_line(&mut line).await {
            Ok(0) => {
                return Err(anyhow::anyhow!("Client disconnected during authentication"));
            }
            Ok(_) => {
                let line = line.trim();
                debug!("IMAP AUTHENTICATE received credentials");

                // Decode base64 credentials
                // PLAIN format: \0username\0password (authorization-id\0authentication-id\0password)
                use base64::{Engine as _, engine::general_purpose::STANDARD};
                
                let decoded = match STANDARD.decode(line) {
                    Ok(d) => d,
                    Err(_) => {
                        return self
                            .send_line(&format!("{} NO Invalid base64 encoding", tag))
                            .await;
                    }
                };

                // Parse the PLAIN credentials (split by null bytes)
                let parts: Vec<&[u8]> = decoded.split(|&b| b == 0).collect();
                
                // PLAIN format: authzid\0authcid\0password (authzid may be empty)
                let (username, password) = if parts.len() >= 3 {
                    // Use authcid (parts[1]) as username, parts[2] as password
                    let username = String::from_utf8_lossy(parts[1]).to_string();
                    let password = String::from_utf8_lossy(parts[2]).to_string();
                    (username, password)
                } else if parts.len() == 2 {
                    // Fallback: just username and password
                    let username = String::from_utf8_lossy(parts[0]).to_string();
                    let password = String::from_utf8_lossy(parts[1]).to_string();
                    (username, password)
                } else {
                    return self
                        .send_line(&format!("{} NO Invalid PLAIN credentials format", tag))
                        .await;
                };

                debug!("IMAP AUTHENTICATE PLAIN for user: {}", username);

                // Extract just the local part if domain is included
                let mailbox_name = if username.contains('@') {
                    username.split('@').next().unwrap_or(&username)
                } else {
                    &username
                };

                // Verify credentials against storage
                match self
                    .storage
                    .verify_mailbox_password(mailbox_name, &password)
                    .await
                {
                    Ok(true) => {
                        self.state = ImapState::Authenticated;
                        self.authenticated_user = Some(mailbox_name.to_string());
                        info!("IMAP user authenticated via PLAIN: {}", mailbox_name);
                        self.send_line(&format!("{} OK AUTHENTICATE completed", tag))
                            .await
                    }
                    Ok(false) => {
                        warn!("IMAP AUTHENTICATE failed for user: {}", username);
                        self.send_line(&format!("{} NO AUTHENTICATE failed", tag))
                            .await
                    }
                    Err(e) => {
                        error!("IMAP AUTHENTICATE error: {}", e);
                        self.send_line(&format!("{} NO AUTHENTICATE failed", tag))
                            .await
                    }
                }
            }
            Err(e) => {
                error!("IMAP read error during AUTHENTICATE: {}", e);
                Err(anyhow::anyhow!("Read error during authentication"))
            }
        }
    }

    async fn cmd_login(&mut self, tag: &str, args: &str) -> Result<()> {
        // Parse username and password from args
        // Format: LOGIN username password
        // Username/password may be quoted
        let (username, password) = match parse_login_args(args) {
            Some((u, p)) => (u, p),
            None => {
                self.send_line(&format!("{} BAD Invalid LOGIN arguments", tag))
                    .await?;
                return Ok(());
            }
        };

        debug!("IMAP LOGIN attempt for user: {}", username);

        // The username should be the mailbox address (e.g., "user" or "user@domain.com")
        // Extract just the local part if domain is included
        let mailbox_name = if username.contains('@') {
            username.split('@').next().unwrap_or(&username)
        } else {
            &username
        };

        // Verify credentials against storage
        match self
            .storage
            .verify_mailbox_password(mailbox_name, &password)
            .await
        {
            Ok(true) => {
                self.state = ImapState::Authenticated;
                self.authenticated_user = Some(mailbox_name.to_string());
                info!("IMAP user authenticated: {}", mailbox_name);
                self.send_line(&format!("{} OK LOGIN completed", tag)).await
            }
            Ok(false) => {
                warn!("IMAP authentication failed for user: {}", username);
                self.send_line(&format!("{} NO LOGIN failed", tag)).await
            }
            Err(e) => {
                error!("IMAP authentication error: {}", e);
                self.send_line(&format!("{} NO LOGIN failed", tag)).await
            }
        }
    }

    async fn cmd_list(&mut self, tag: &str, args: &str) -> Result<()> {
        if self.state == ImapState::NotAuthenticated {
            return self
                .send_line(&format!("{} NO Not authenticated", tag))
                .await;
        }

        // Parse reference and mailbox pattern
        let (_reference, pattern) = parse_list_args(args);

        // If pattern is empty or %, list INBOX
        if pattern.is_empty() || pattern == "%" || pattern == "*" {
            // List the user's INBOX (their mailbox)
            self.send_line("* LIST (\\HasNoChildren) \"/\" \"INBOX\"")
                .await?;
        }

        self.send_line(&format!("{} OK LIST completed", tag)).await
    }

    async fn cmd_lsub(&mut self, tag: &str, args: &str) -> Result<()> {
        if self.state == ImapState::NotAuthenticated {
            return self
                .send_line(&format!("{} NO Not authenticated", tag))
                .await;
        }

        // LSUB is similar to LIST but for subscribed mailboxes
        let (_reference, pattern) = parse_list_args(args);

        if pattern.is_empty() || pattern == "%" || pattern == "*" {
            self.send_line("* LSUB (\\HasNoChildren) \"/\" \"INBOX\"")
                .await?;
        }

        self.send_line(&format!("{} OK LSUB completed", tag)).await
    }

    async fn cmd_select(&mut self, tag: &str, args: &str) -> Result<()> {
        if self.state == ImapState::NotAuthenticated {
            return self
                .send_line(&format!("{} NO Not authenticated", tag))
                .await;
        }

        let mailbox = unquote(args.trim());

        // Only support INBOX for now
        if mailbox.to_uppercase() != "INBOX" {
            return self
                .send_line(&format!("{} NO Mailbox does not exist", tag))
                .await;
        }

        // Get email count for the authenticated user
        let user = match &self.authenticated_user {
            Some(u) => u.clone(),
            None => {
                return self
                    .send_line(&format!("{} NO Not authenticated", tag))
                    .await;
            }
        };

        // Build the full email address
        let full_address = format!("{}@{}", user, self.domain_name);
        let emails = self
            .storage
            .get_emails_for_address(&full_address)
            .await
            .unwrap_or_default();

        let count = emails.len();

        self.state = ImapState::Selected(mailbox.to_string());

        // Send mailbox information
        self.send_line(&format!("* {} EXISTS", count)).await?;
        self.send_line("* 0 RECENT").await?;
        self.send_line("* OK [UIDVALIDITY 1] UIDs valid").await?;
        self.send_line(&format!("* OK [UIDNEXT {}] Predicted next UID", count + 1))
            .await?;
        self.send_line("* FLAGS (\\Seen \\Answered \\Flagged \\Deleted \\Draft)")
            .await?;
        self.send_line("* OK [PERMANENTFLAGS ()] No permanent flags permitted")
            .await?;

        self.send_line(&format!("{} OK [READ-ONLY] SELECT completed", tag))
            .await
    }

    async fn cmd_examine(&mut self, tag: &str, args: &str) -> Result<()> {
        // EXAMINE is like SELECT but read-only (which our SELECT already is)
        self.cmd_select(tag, args).await
    }

    async fn cmd_fetch(&mut self, tag: &str, args: &str) -> Result<()> {
        if !matches!(self.state, ImapState::Selected(_)) {
            return self
                .send_line(&format!("{} NO No mailbox selected", tag))
                .await;
        }

        // Parse sequence set and data items
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() < 2 {
            return self
                .send_line(&format!("{} BAD Invalid FETCH arguments", tag))
                .await;
        }

        let sequence_set = parts[0];
        let data_items = parts[1];

        self.do_fetch(tag, sequence_set, data_items, false).await
    }

    async fn cmd_uid(&mut self, tag: &str, args: &str) -> Result<()> {
        if !matches!(self.state, ImapState::Selected(_)) {
            return self
                .send_line(&format!("{} NO No mailbox selected", tag))
                .await;
        }

        // UID command wraps other commands
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.is_empty() {
            return self
                .send_line(&format!("{} BAD Invalid UID arguments", tag))
                .await;
        }

        let subcommand = parts[0].to_uppercase();
        let subargs = parts.get(1).copied().unwrap_or("");

        match subcommand.as_str() {
            "FETCH" => {
                let subparts: Vec<&str> = subargs.splitn(2, ' ').collect();
                if subparts.len() < 2 {
                    return self
                        .send_line(&format!("{} BAD Invalid UID FETCH arguments", tag))
                        .await;
                }
                self.do_fetch(tag, subparts[0], subparts[1], true).await
            }
            "SEARCH" => self.do_search(tag, subargs, true).await,
            _ => {
                self.send_line(&format!("{} BAD Unknown UID subcommand", tag))
                    .await
            }
        }
    }

    async fn do_fetch(
        &mut self,
        tag: &str,
        sequence_set: &str,
        data_items: &str,
        use_uid: bool,
    ) -> Result<()> {
        let user = match &self.authenticated_user {
            Some(u) => u.clone(),
            None => {
                return self
                    .send_line(&format!("{} NO Not authenticated", tag))
                    .await;
            }
        };

        let full_address = format!("{}@{}", user, self.domain_name);
        let emails = self
            .storage
            .get_emails_for_address(&full_address)
            .await
            .unwrap_or_default();

        // Parse sequence set
        let indices = parse_sequence_set(sequence_set, emails.len(), use_uid);

        // Parse what data items to fetch
        let items = data_items.to_uppercase();
        let want_envelope = items.contains("ENVELOPE");
        let want_body = items.contains("BODY") || items.contains("RFC822");
        let want_flags = items.contains("FLAGS");
        let want_uid = items.contains("UID") || use_uid;
        let want_internaldate = items.contains("INTERNALDATE");

        for idx in indices {
            if idx == 0 || idx > emails.len() {
                continue;
            }

            let email = &emails[idx - 1];
            let mut response_parts = Vec::new();

            if want_flags {
                response_parts.push("FLAGS ()".to_string());
            }

            if want_uid {
                response_parts.push(format!("UID {}", idx));
            }

            if want_internaldate {
                let date = email.timestamp.format("%d-%b-%Y %H:%M:%S %z");
                response_parts.push(format!("INTERNALDATE \"{}\"", date));
            }

            if want_envelope {
                let envelope = format!(
                    "ENVELOPE (\"{}\" \"{}\" ((NIL NIL \"{}\" \"{}\")) ((NIL NIL \"{}\" \"{}\")) ((NIL NIL \"{}\" \"{}\")) ((NIL NIL \"{}\" \"{}\")) NIL NIL NIL NIL)",
                    email.timestamp.format("%a, %d %b %Y %H:%M:%S %z"),
                    escape_imap_string(&email.subject),
                    extract_local_part(&email.from),
                    extract_domain(&email.from),
                    extract_local_part(&email.from),
                    extract_domain(&email.from),
                    extract_local_part(&email.from),
                    extract_domain(&email.from),
                    extract_local_part(&email.to),
                    extract_domain(&email.to),
                );
                response_parts.push(envelope);
            }

            if want_body {
                // Build RFC822-style message
                let rfc822 = if let Some(raw) = &email.raw {
                    raw.clone()
                } else {
                    format!(
                        "From: {}\r\nTo: {}\r\nSubject: {}\r\nDate: {}\r\nMessage-ID: <{}@{}>\r\n\r\n{}",
                        email.from,
                        email.to,
                        email.subject,
                        email.timestamp.format("%a, %d %b %Y %H:%M:%S %z"),
                        email.id,
                        self.domain_name,
                        email.body
                    )
                };

                let body_len = rfc822.len();
                response_parts.push(format!("BODY[] {{{}}}\r\n{}", body_len, rfc822));
            }

            let response = format!("* {} FETCH ({})", idx, response_parts.join(" "));
            self.send_line(&response).await?;
        }

        let cmd_name = if use_uid { "UID FETCH" } else { "FETCH" };
        self.send_line(&format!("{} OK {} completed", tag, cmd_name))
            .await
    }

    async fn cmd_search(&mut self, tag: &str, args: &str) -> Result<()> {
        if !matches!(self.state, ImapState::Selected(_)) {
            return self
                .send_line(&format!("{} NO No mailbox selected", tag))
                .await;
        }

        self.do_search(tag, args, false).await
    }

    async fn do_search(&mut self, tag: &str, args: &str, use_uid: bool) -> Result<()> {
        let user = match &self.authenticated_user {
            Some(u) => u.clone(),
            None => {
                return self
                    .send_line(&format!("{} NO Not authenticated", tag))
                    .await;
            }
        };

        let full_address = format!("{}@{}", user, self.domain_name);
        let emails = self
            .storage
            .get_emails_for_address(&full_address)
            .await
            .unwrap_or_default();

        // Simple search implementation - just return all message numbers for now
        // A real implementation would parse the search criteria
        let args_upper = args.to_uppercase();

        let results: Vec<usize> = if args_upper.contains("ALL") || args_upper.is_empty() {
            (1..=emails.len()).collect()
        } else {
            // For any other search, return all for now
            // TODO: Implement proper search criteria parsing
            (1..=emails.len()).collect()
        };

        if results.is_empty() {
            self.send_line("* SEARCH").await?;
        } else {
            let result_str = results
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            self.send_line(&format!("* SEARCH {}", result_str)).await?;
        }

        let cmd_name = if use_uid { "UID SEARCH" } else { "SEARCH" };
        self.send_line(&format!("{} OK {} completed", tag, cmd_name))
            .await
    }

    async fn cmd_close(&mut self, tag: &str) -> Result<()> {
        if !matches!(self.state, ImapState::Selected(_)) {
            return self
                .send_line(&format!("{} NO No mailbox selected", tag))
                .await;
        }

        self.state = ImapState::Authenticated;
        self.send_line(&format!("{} OK CLOSE completed", tag)).await
    }
}

// Helper functions

/// Parse LOGIN arguments (username and password, possibly quoted)
fn parse_login_args(args: &str) -> Option<(String, String)> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let chars = args.chars().peekable();

    for c in chars {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if parts.len() >= 2 {
        Some((parts[0].clone(), parts[1].clone()))
    } else {
        None
    }
}

/// Parse LIST/LSUB arguments (reference and mailbox pattern)
fn parse_list_args(args: &str) -> (String, String) {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    let reference = unquote(parts.first().copied().unwrap_or(""));
    let pattern = unquote(parts.get(1).copied().unwrap_or(""));
    (reference.to_string(), pattern.to_string())
}

/// Remove surrounding quotes from a string
fn unquote(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Parse IMAP sequence set (e.g., "1", "1:5", "1,3,5", "*")
fn parse_sequence_set(set: &str, total: usize, _use_uid: bool) -> Vec<usize> {
    let mut result = Vec::new();

    for part in set.split(',') {
        let part = part.trim();
        if part == "*" {
            if total > 0 {
                result.push(total);
            }
        } else if part.contains(':') {
            let bounds: Vec<&str> = part.split(':').collect();
            if bounds.len() == 2 {
                let start = if bounds[0] == "*" {
                    total
                } else {
                    bounds[0].parse().unwrap_or(1)
                };
                let end = if bounds[1] == "*" {
                    total
                } else {
                    bounds[1].parse().unwrap_or(total)
                };
                let (start, end) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };
                for i in start..=end {
                    if i >= 1 && i <= total {
                        result.push(i);
                    }
                }
            }
        } else if let Ok(num) = part.parse::<usize>() {
            if num >= 1 && num <= total {
                result.push(num);
            }
        }
    }

    result
}

/// Escape special characters for IMAP strings
fn escape_imap_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Extract local part from email address
fn extract_local_part(email: &str) -> &str {
    email.split('@').next().unwrap_or(email)
}

/// Extract domain from email address
fn extract_domain(email: &str) -> &str {
    email.split('@').nth(1).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_login_args() {
        assert_eq!(
            parse_login_args("user password"),
            Some(("user".to_string(), "password".to_string()))
        );
        assert_eq!(
            parse_login_args("\"user\" \"password\""),
            Some(("user".to_string(), "password".to_string()))
        );
        assert_eq!(
            parse_login_args("\"user@domain.com\" \"pass word\""),
            Some(("user@domain.com".to_string(), "pass word".to_string()))
        );
        assert_eq!(parse_login_args("onlyuser"), None);
    }

    #[test]
    fn test_unquote() {
        assert_eq!(unquote("\"hello\""), "hello");
        assert_eq!(unquote("hello"), "hello");
        assert_eq!(unquote("\"\""), "");
        assert_eq!(unquote(" \"test\" "), "test");
    }

    #[test]
    fn test_parse_sequence_set() {
        assert_eq!(parse_sequence_set("1", 10, false), vec![1]);
        assert_eq!(parse_sequence_set("1:3", 10, false), vec![1, 2, 3]);
        assert_eq!(parse_sequence_set("1,3,5", 10, false), vec![1, 3, 5]);
        assert_eq!(parse_sequence_set("*", 10, false), vec![10]);
        assert_eq!(parse_sequence_set("1:*", 5, false), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_extract_email_parts() {
        assert_eq!(extract_local_part("user@domain.com"), "user");
        assert_eq!(extract_domain("user@domain.com"), "domain.com");
        assert_eq!(extract_local_part("justuser"), "justuser");
        assert_eq!(extract_domain("justuser"), "");
    }
}
