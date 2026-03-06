use anyhow::{Context, Result};
use mail_auth::common::crypto::{RsaKey, Sha256};
use mail_auth::common::headers::HeaderWriter;
use mail_auth::dkim::{Canonicalization, DkimSigner as MailAuthDkimSigner};
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::pkcs8::LineEnding;
use rsa::RsaPrivateKey;
use std::path::Path;

/// Generate DKIM keys and print DNS records to stdout
pub fn generate_keys(domain: &str, selector: &str, output: &str) -> Result<()> {
    println!("Generating 2048-bit RSA keypair for DKIM...\n");

    let mut rng = rsa::rand_core::OsRng;
    let private_key =
        RsaPrivateKey::new(&mut rng, 2048).context("Failed to generate RSA private key")?;

    // Write private key PEM to file
    let pem = private_key
        .to_pkcs1_pem(LineEnding::LF)
        .context("Failed to encode private key as PEM")?;
    std::fs::write(output, pem.as_bytes()).context("Failed to write private key file")?;

    // Set restrictive permissions on the key file
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o600))?;
    }

    println!("Private key written to: {}\n", output);

    // Extract public key DER and base64-encode
    let public_key = private_key.to_public_key();
    let pub_der = public_key
        .to_pkcs1_der()
        .context("Failed to encode public key as DER")?;
    let pub_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        pub_der.as_bytes(),
    );

    println!("=== DNS Records ===\n");

    println!("1. DKIM TXT Record:");
    println!("   {selector}._domainkey.{domain} IN TXT \"v=DKIM1; k=rsa; p={pub_b64}\"\n");

    println!("2. SPF TXT Record:");
    println!("   {domain} IN TXT \"v=spf1 a mx ip4:<YOUR_SERVER_IP> ~all\"\n");

    println!("3. DMARC TXT Record:");
    println!("   _dmarc.{domain} IN TXT \"v=DMARC1; p=none; rua=mailto:postmaster@{domain}\"\n");

    println!("4. MTA-STS (optional):");
    println!("   _mta-sts.{domain} IN TXT \"v=STSv1; id=<TIMESTAMP>\"");
    println!("   Host a policy file at https://mta-sts.{domain}/.well-known/mta-sts.txt\n");

    println!("=== Setup Instructions ===\n");
    println!("1. Add the DNS records above to your domain's DNS zone");
    println!("2. Replace <YOUR_SERVER_IP> in the SPF record with your server's public IP");
    println!("3. Set these environment variables to enable outbound email:");
    println!("   AUTH_ENABLED=true  (required - prevents open relay)");
    println!("   OUTBOUND_ENABLED=true");
    println!("   DKIM_PRIVATE_KEY_PATH={output}");
    println!("   DKIM_SELECTOR={selector}");
    println!("   DKIM_DOMAIN={domain}");
    println!("4. Optionally configure SMTP relay:");
    println!("   SMTP_RELAY_HOST=smtp.example.com");
    println!("   SMTP_RELAY_PORT=587");
    println!("   SMTP_RELAY_USERNAME=user");
    println!("   SMTP_RELAY_PASSWORD=pass");
    println!("5. Restart the server");

    Ok(())
}

/// Runtime DKIM signer
pub struct DkimSigner {
    private_key_pem: String,
    selector: String,
    domain: String,
}

impl DkimSigner {
    /// Load a DKIM signer from a PEM private key file
    pub fn from_pem_file(path: &Path, selector: String, domain: String) -> Result<Self> {
        let pem = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read DKIM private key from {}", path.display()))?;
        Ok(Self {
            private_key_pem: pem,
            selector,
            domain,
        })
    }

    /// Sign a raw RFC 5322 message, returning the message with DKIM-Signature prepended
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        // Parse PEM to DER
        let der = rustls_pemfile::private_key(&mut self.private_key_pem.as_bytes())
            .context("Failed to parse PEM")?
            .context("No private key found in PEM")?;

        let pk = RsaKey::<Sha256>::from_key_der(der).context("Failed to parse DKIM private key")?;

        let signer = MailAuthDkimSigner::from_key(pk)
            .domain(&self.domain)
            .selector(&self.selector)
            .headers([
                "From",
                "To",
                "Subject",
                "Date",
                "Message-ID",
                "MIME-Version",
                "Content-Type",
            ])
            .header_canonicalization(Canonicalization::Relaxed)
            .body_canonicalization(Canonicalization::Relaxed);

        let signature = signer
            .sign(message)
            .context("Failed to sign message with DKIM")?;

        let dkim_header = signature.to_header();

        let mut signed_message = Vec::with_capacity(dkim_header.len() + message.len());
        signed_message.extend_from_slice(dkim_header.as_bytes());
        signed_message.extend_from_slice(message);

        Ok(signed_message)
    }
}
