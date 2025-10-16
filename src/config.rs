use anyhow::Result;
use std::path::PathBuf;

/// Type alias for SSL certificate data (certificates, private_key)
pub type SslCertificates = (Vec<Vec<u8>>, Vec<u8>);

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub smtp_port: u16,
    pub smtp_starttls_port: u16, // Port 587 for STARTTLS (explicit TLS)
    pub smtp_ssl_port: u16,      // Port 465 for SMTPS (implicit TLS)
    pub api_port: u16,
    pub database_url: String,
    pub smtp_ssl: SmtpSslConfig,
    pub domain_name: String,
    pub email_retention_hours: Option<i64>,
    pub reject_non_domain_emails: bool,
    pub mcp_enabled: bool,
    pub mcp_port: u16,
}

/// SMTP SSL/TLS configuration for Let's Encrypt certificates
#[derive(Debug, Clone)]
pub struct SmtpSslConfig {
    pub enabled: bool,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        // Load .env file if it exists (don't fail if it doesn't)
        let _ = dotenvy::dotenv();

        // Non-TLS SMTP port (always listening)
        let smtp_port = std::env::var("SMTP_PORT")
            .unwrap_or_else(|_| "2525".to_string())
            .parse()?;

        // STARTTLS port (explicit TLS upgrade on port 587) - only used if SSL enabled
        let smtp_starttls_port = std::env::var("SMTP_STARTTLS_PORT")
            .unwrap_or_else(|_| "587".to_string())
            .parse()?;

        // SMTPS port (implicit TLS on port 465) - only used if SSL enabled
        let smtp_ssl_port = std::env::var("SMTP_SSL_PORT")
            .unwrap_or_else(|_| "465".to_string())
            .parse()?;

        let api_port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;

        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:emails.db".to_string());

        let domain_name =
            std::env::var("DOMAIN_NAME").unwrap_or_else(|_| "tempmail.local".to_string());

        let email_retention_hours = std::env::var("EMAIL_RETENTION_HOURS")
            .ok()
            .and_then(|s| s.parse().ok());

        let reject_non_domain_emails = std::env::var("REJECT_NON_DOMAIN_EMAILS")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let mcp_enabled = std::env::var("MCP_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let mcp_port = std::env::var("MCP_PORT")
            .unwrap_or_else(|_| "3001".to_string())
            .parse()?;

        // SMTP SSL configuration for Let's Encrypt
        let smtp_ssl_enabled = std::env::var("SMTP_SSL_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let smtp_ssl = if smtp_ssl_enabled {
            let cert_path = std::env::var("SMTP_SSL_CERT_PATH").map(PathBuf::from).ok();
            let key_path = std::env::var("SMTP_SSL_KEY_PATH").map(PathBuf::from).ok();

            if cert_path.is_none() || key_path.is_none() {
                anyhow::bail!("SMTP_SSL_ENABLED is true but SMTP_SSL_CERT_PATH and SMTP_SSL_KEY_PATH must be set");
            }

            SmtpSslConfig {
                enabled: true,
                cert_path,
                key_path,
            }
        } else {
            SmtpSslConfig {
                enabled: false,
                cert_path: None,
                key_path: None,
            }
        };

        Ok(Config {
            smtp_port,
            smtp_starttls_port,
            smtp_ssl_port,
            api_port,
            database_url,
            smtp_ssl,
            domain_name,
            email_retention_hours,
            reject_non_domain_emails,
            mcp_enabled,
            mcp_port,
        })
    }
}

impl SmtpSslConfig {
    /// Load SSL certificates from the filesystem
    pub fn load_certificates(&self) -> Result<Option<SslCertificates>> {
        if !self.enabled {
            return Ok(None);
        }

        let cert_path = self
            .cert_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Certificate path not set"))?;
        let key_path = self
            .key_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Key path not set"))?;

        // Read certificate file
        let cert_file = std::fs::read(cert_path)?;
        let certs_raw =
            rustls_pemfile::certs(&mut &cert_file[..]).collect::<Result<Vec<_>, _>>()?;
        let certs: Vec<Vec<u8>> = certs_raw
            .iter()
            .map(|cert| cert.as_ref().to_vec())
            .collect();

        // Read private key file
        let key_file = std::fs::read(key_path)?;
        let key = rustls_pemfile::private_key(&mut &key_file[..])?
            .ok_or_else(|| anyhow::anyhow!("No private key found in key file"))?;

        Ok(Some((certs, key.secret_der().to_vec())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Load configuration from environment variables without loading .env file
    /// This is used for tests to avoid interference from .env files
    fn from_env_test() -> Result<Config> {
        // Non-TLS SMTP port (always listening)
        let smtp_port = std::env::var("SMTP_PORT")
            .unwrap_or_else(|_| "2525".to_string())
            .parse()?;

        // STARTTLS port
        let smtp_starttls_port = std::env::var("SMTP_STARTTLS_PORT")
            .unwrap_or_else(|_| "587".to_string())
            .parse()?;

        // SSL/TLS port
        let smtp_ssl_port = std::env::var("SMTP_SSL_PORT")
            .unwrap_or_else(|_| "465".to_string())
            .parse()?;

        // API port
        let api_port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;

        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:emails.db".to_string());

        let domain_name =
            std::env::var("DOMAIN_NAME").unwrap_or_else(|_| "tempmail.local".to_string());

        let email_retention_hours = std::env::var("EMAIL_RETENTION_HOURS")
            .ok()
            .and_then(|s| s.parse().ok());

        let reject_non_domain_emails = std::env::var("REJECT_NON_DOMAIN_EMAILS")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let smtp_ssl_enabled = std::env::var("SMTP_SSL_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let smtp_ssl = if smtp_ssl_enabled {
            let cert_path = std::env::var("SMTP_SSL_CERT_PATH").map(PathBuf::from).ok();
            let key_path = std::env::var("SMTP_SSL_KEY_PATH").map(PathBuf::from).ok();

            if cert_path.is_none() || key_path.is_none() {
                anyhow::bail!("SMTP_SSL_ENABLED is true but SMTP_SSL_CERT_PATH and SMTP_SSL_KEY_PATH must be set");
            }

            SmtpSslConfig {
                enabled: true,
                cert_path,
                key_path,
            }
        } else {
            SmtpSslConfig {
                enabled: false,
                cert_path: None,
                key_path: None,
            }
        };

        let mcp_enabled = std::env::var("MCP_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let mcp_port = std::env::var("MCP_PORT")
            .unwrap_or_else(|_| "3001".to_string())
            .parse()
            .unwrap_or(3001);

        Ok(Config {
            smtp_port,
            smtp_starttls_port,
            smtp_ssl_port,
            api_port,
            database_url,
            domain_name,
            email_retention_hours,
            reject_non_domain_emails,
            smtp_ssl,
            mcp_enabled,
            mcp_port,
        })
    }

    fn clear_all_env_vars() {
        // Clear all environment variables that might interfere with tests
        env::remove_var("SMTP_PORT");
        env::remove_var("SMTP_STARTTLS_PORT");
        env::remove_var("SMTP_SSL_PORT");
        env::remove_var("API_PORT");
        env::remove_var("DATABASE_URL");
        env::remove_var("DOMAIN_NAME");
        env::remove_var("EMAIL_RETENTION_HOURS");
        env::remove_var("REJECT_NON_DOMAIN_EMAILS");
        env::remove_var("SMTP_SSL_ENABLED");
        env::remove_var("SMTP_SSL_CERT_PATH");
        env::remove_var("SMTP_SSL_KEY_PATH");
        env::remove_var("MCP_ENABLED");
        env::remove_var("MCP_PORT");
    }

    #[test]
    fn test_config_from_env_defaults() {
        clear_all_env_vars();
        let config = from_env_test().unwrap();

        assert_eq!(config.smtp_port, 2525);
        assert_eq!(config.smtp_starttls_port, 587);
        assert_eq!(config.smtp_ssl_port, 465);
        assert_eq!(config.api_port, 3000);
        assert_eq!(config.database_url, "sqlite:emails.db");
        assert_eq!(config.domain_name, "tempmail.local");
        assert_eq!(config.email_retention_hours, None);
        assert_eq!(config.reject_non_domain_emails, false);
        assert_eq!(config.smtp_ssl.enabled, false);
        assert_eq!(config.mcp_enabled, false);
        assert_eq!(config.mcp_port, 3001);
        
        // Clean up after test
        clear_all_env_vars();
    }

    #[test]
    fn test_config_from_env_custom() {
        // Clear all environment variables first to avoid interference
        clear_all_env_vars();
        env::set_var("SMTP_PORT", "2526");
        env::set_var("SMTP_STARTTLS_PORT", "588");
        env::set_var("SMTP_SSL_PORT", "466");
        env::set_var("API_PORT", "3001");
        env::set_var("DATABASE_URL", "sqlite:test.db");
        env::set_var("DOMAIN_NAME", "test.local");
        env::set_var("EMAIL_RETENTION_HOURS", "24");
        env::set_var("REJECT_NON_DOMAIN_EMAILS", "true");
        env::set_var("SMTP_SSL_ENABLED", "true");
        env::set_var("SMTP_SSL_CERT_PATH", "/path/to/cert.pem");
        env::set_var("SMTP_SSL_KEY_PATH", "/path/to/key.pem");
        env::set_var("MCP_ENABLED", "true");
        env::set_var("MCP_PORT", "3002");

        let config = from_env_test().unwrap();

        assert_eq!(config.smtp_port, 2526);
        assert_eq!(config.smtp_starttls_port, 588);
        assert_eq!(config.smtp_ssl_port, 466);
        assert_eq!(config.api_port, 3001);
        assert_eq!(config.database_url, "sqlite:test.db");
        assert_eq!(config.domain_name, "test.local");
        assert_eq!(config.email_retention_hours, Some(24));
        assert_eq!(config.reject_non_domain_emails, true);
        assert_eq!(config.smtp_ssl.enabled, true);
        assert_eq!(
            config.smtp_ssl.cert_path,
            Some(std::path::PathBuf::from("/path/to/cert.pem"))
        );
        assert_eq!(
            config.smtp_ssl.key_path,
            Some(std::path::PathBuf::from("/path/to/key.pem"))
        );
        assert_eq!(config.mcp_enabled, true);
        assert_eq!(config.mcp_port, 3002);
        
        // Clean up after test
        clear_all_env_vars();
    }

    #[test]
    fn test_config_ssl_enabled_without_cert_paths() {
        clear_all_env_vars();
        env::set_var("SMTP_SSL_ENABLED", "true");
        env::remove_var("SMTP_SSL_CERT_PATH");
        env::remove_var("SMTP_SSL_KEY_PATH");

        let result = from_env_test();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("SMTP_SSL_CERT_PATH and SMTP_SSL_KEY_PATH must be set"));
        
        // Clean up after test
        clear_all_env_vars();
    }

    #[test]
    fn test_config_invalid_port() {
        clear_all_env_vars();
        env::set_var("SMTP_PORT", "invalid");

        let result = from_env_test();
        assert!(result.is_err());
        
        // Clean up after test
        clear_all_env_vars();
    }

    #[test]
    fn test_config_invalid_retention_hours() {
        clear_all_env_vars();
        env::set_var("EMAIL_RETENTION_HOURS", "invalid");

        let config = from_env_test().unwrap();
        assert_eq!(config.email_retention_hours, None);
        
        // Clean up after test
        clear_all_env_vars();
    }

    #[test]
    fn test_config_invalid_reject_non_domain_emails() {
        clear_all_env_vars();
        env::set_var("REJECT_NON_DOMAIN_EMAILS", "invalid");

        let config = from_env_test().unwrap();
        assert_eq!(config.reject_non_domain_emails, false);
        
        // Clean up after test
        clear_all_env_vars();
    }

    #[test]
    fn test_smtp_ssl_config_disabled() {
        let ssl_config = SmtpSslConfig {
            enabled: false,
            cert_path: None,
            key_path: None,
        };

        let result = ssl_config.load_certificates().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_smtp_ssl_config_enabled_without_paths() {
        let ssl_config = SmtpSslConfig {
            enabled: true,
            cert_path: None,
            key_path: None,
        };

        let result = ssl_config.load_certificates();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Certificate path not set"));
    }

    #[test]
    fn test_smtp_ssl_config_enabled_with_nonexistent_files() {
        let ssl_config = SmtpSslConfig {
            enabled: true,
            cert_path: Some(std::path::PathBuf::from("/nonexistent/cert.pem")),
            key_path: Some(std::path::PathBuf::from("/nonexistent/key.pem")),
        };

        let result = ssl_config.load_certificates();
        assert!(result.is_err());
    }

    #[test]
    fn test_smtp_ssl_config_with_valid_files() {
        let temp_dir = std::env::temp_dir();
        let cert_path = temp_dir.join("cert.pem");
        let key_path = temp_dir.join("key.pem");

        // Create dummy certificate and key files
        std::fs::write(
            &cert_path,
            "-----BEGIN CERTIFICATE-----\nMOCK_CERT\n-----END CERTIFICATE-----",
        )
        .unwrap();
        std::fs::write(
            &key_path,
            "-----BEGIN PRIVATE KEY-----\nMOCK_KEY\n-----END PRIVATE KEY-----",
        )
        .unwrap();

        let ssl_config = SmtpSslConfig {
            enabled: true,
            cert_path: Some(cert_path),
            key_path: Some(key_path),
        };

        // This will fail because the files don't contain valid PEM data, but we can test the path logic
        let result = ssl_config.load_certificates();
        assert!(result.is_err()); // Expected to fail due to invalid PEM content
    }
}
