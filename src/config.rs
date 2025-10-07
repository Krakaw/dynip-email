use anyhow::Result;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub smtp_port: u16,
    pub smtp_starttls_port: u16,  // Port 587 for STARTTLS (explicit TLS)
    pub smtp_ssl_port: u16,       // Port 465 for SMTPS (implicit TLS)
    pub api_port: u16,
    pub database_url: String,
    pub smtp_ssl: SmtpSslConfig,
    pub domain_name: String,
    pub email_retention_hours: Option<i64>,
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
        
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:emails.db".to_string());
        
        let domain_name = std::env::var("DOMAIN_NAME")
            .unwrap_or_else(|_| "tempmail.local".to_string());
        
        let email_retention_hours = std::env::var("EMAIL_RETENTION_HOURS")
            .ok()
            .and_then(|s| s.parse().ok());
        
        // SMTP SSL configuration for Let's Encrypt
        let smtp_ssl_enabled = std::env::var("SMTP_SSL_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);
        
        let smtp_ssl = if smtp_ssl_enabled {
            let cert_path = std::env::var("SMTP_SSL_CERT_PATH")
                .map(PathBuf::from)
                .ok();
            let key_path = std::env::var("SMTP_SSL_KEY_PATH")
                .map(PathBuf::from)
                .ok();
            
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
        })
    }
}

impl SmtpSslConfig {
    /// Load SSL certificates from the filesystem
    pub fn load_certificates(&self) -> Result<Option<(Vec<Vec<u8>>, Vec<u8>)>> {
        if !self.enabled {
            return Ok(None);
        }
        
        let cert_path = self.cert_path.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Certificate path not set"))?;
        let key_path = self.key_path.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Key path not set"))?;
        
        // Read certificate file
        let cert_file = std::fs::read(cert_path)?;
        let certs_raw = rustls_pemfile::certs(&mut &cert_file[..])
            .collect::<Result<Vec<_>, _>>()?;
        let certs: Vec<Vec<u8>> = certs_raw.iter()
            .map(|cert| cert.as_ref().to_vec())
            .collect();
        
        // Read private key file
        let key_file = std::fs::read(key_path)?;
        let key = rustls_pemfile::private_key(&mut &key_file[..])?
            .ok_or_else(|| anyhow::anyhow!("No private key found in key file"))?;
        
        Ok(Some((certs, key.secret_der().to_vec())))
    }
}

