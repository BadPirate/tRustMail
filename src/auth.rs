use crate::config::DomainConfig;
use crate::db;
use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signer};
use lettre::Message;
use rand_core::OsRng;
use sqlx::PgPool;
use std::error::Error;
use tracing::info;
use sha2::{Sha256, Digest};

// DKIM signing functions
pub async fn sign_email_with_dkim(
    pool: &PgPool,
    message: &Message,
    domain_config: &DomainConfig,
) -> Result<Message, Box<dyn Error>> {
    // Get or create DKIM keys for the domain
    let domain_key = get_domain_key(pool, &domain_config.domain, &domain_config.dkim_selector).await?;
    
    // Parse the private key from the stored key
    let secret_key_bytes = base64::decode(&domain_key.private_key)?;
    let secret_key = SecretKey::from_bytes(&secret_key_bytes)?;
    let public_key_bytes = base64::decode(&domain_key.public_key)?;
    let public_key = PublicKey::from_bytes(&public_key_bytes)?;
    let keypair = Keypair { secret: secret_key, public: public_key };
    
    // Get the email as bytes
    let email_bytes = message.formatted();
    
    // Create DKIM signature header
    // This is a simplified implementation
    let dkim_header = format!(
        "DKIM-Signature: v=1; a=ed25519-sha256; c=relaxed/relaxed; d={}; s={};\r\n\tt={}; bh={}; h=from:to:subject:date; b={}",
        domain_config.domain,
        domain_config.dkim_selector,
        chrono::Utc::now().timestamp(),
        {
            // Use sha2 library to create a SHA-256 hash
            let mut hasher = Sha256::new();
            // Directly use the bytes from the email content
            let bytes: &[u8] = email_bytes.as_ref();
            hasher.update(bytes);
            base64::encode(hasher.finalize())
        },
        base64::encode(keypair.sign(&email_bytes).to_bytes())
    );
    
    // Add DKIM header to the email
    // Since we can't directly parse bytes back to Message, we'll construct a new message
    // This is a simplified approach - in a production environment, you'd want a more robust solution
    
    // Create a new message with the DKIM header
    let new_message = message.clone();
    
    // In a real implementation, we would properly add the DKIM header to the email
    // For now, we're returning the original message as a placeholder
    // TODO: Implement proper DKIM header insertion
    
    Ok(new_message)
}

// Get domain key from database or generate a new one
pub async fn get_domain_key(
    pool: &PgPool,
    domain: &str,
    selector: &str,
) -> Result<db::DomainKey, Box<dyn Error>> {
    // Try to fetch existing key
    if let Some(key) = db::get_domain_key(pool, domain, selector).await? {
        return Ok(key);
    }
    
    // No key found, generate a new one
    let mut csprng = OsRng;
    let keypair = Keypair::generate(&mut csprng);
    
    // Store private key securely and public key
    let private_key_b64 = base64::encode(keypair.secret.to_bytes());
    let public_key_str = base64::encode(keypair.public.as_bytes());
    
    // Store in database
    let _key_id = db::store_domain_key(pool, domain, selector, &private_key_b64, &public_key_str).await?;
    
    // Fetch the stored key
    let key = db::get_domain_key(pool, domain, selector)
        .await?
        .ok_or_else(|| "Failed to retrieve newly created domain key".to_string())?;
    
    Ok(key)
}

// Generate keys for all configured domains if they don't exist
pub async fn generate_keys_if_needed(
    pool: &PgPool,
    config: &crate::config::Config,
) -> Result<(), Box<dyn Error>> {
    for domain_config in &config.domains {
        let domain = &domain_config.domain;
        let selector = &domain_config.dkim_selector;
        
        // Check if keys exist
        if db::get_domain_key(pool, domain, selector).await?.is_none() {
            // Generate and store new keys
            info!("Generating new DKIM keys for domain: {} with selector: {}", domain, selector);
            let domain_key = get_domain_key(pool, domain, selector).await?;
            
            // Output DNS configuration
            print_dns_configuration(domain, selector, &domain_key);
        } else {
            info!("DKIM keys already exist for domain: {} with selector: {}", domain, selector);
        }
    }
    
    Ok(())
}

// Print DNS configuration for DKIM, SPF, and DMARC
fn print_dns_configuration(domain: &str, selector: &str, domain_key: &db::DomainKey) {
    println!("\n==== DNS Configuration for domain: {} ====", domain);
    
    // DKIM record
    println!("\n== DKIM Record ==");
    println!("Add this TXT record:");
    println!("{}._domainkey.{} IN TXT \"v=DKIM1; k=ed25519; p={}\"", 
        selector, domain, domain_key.public_key);
    
    // SPF record
    println!("\n== SPF Record ==");
    println!("Add this TXT record:");
    println!("{} IN TXT \"v=spf1 a mx ip4:YOUR_SERVER_IP ~all\"", domain);
    println!("(Replace YOUR_SERVER_IP with your actual server IP address)");
    
    // DMARC record
    println!("\n== DMARC Record ==");
    println!("Add this TXT record:");
    println!("_dmarc.{} IN TXT \"v=DMARC1; p=none; sp=quarantine; rua=mailto:dmarc@{}; ruf=mailto:dmarc@{}; fo=1\"", 
        domain, domain, domain);
    
    println!("\n==== End DNS Configuration ====\n");
}
