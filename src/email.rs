use crate::auth::sign_email_with_dkim;
use crate::config::{Config, RetryConfig};
use crate::db::{self, EmailStatus};
use chrono::{DateTime, Duration, Utc};
use lettre::{
    message::{header::ContentType, Mailbox, MessageBuilder},
    transport::smtp::{
        authentication::Credentials, client::TlsParameters,
        AsyncSmtpTransport,
    },
    AsyncTransport, Message,
};
use sqlx::PgPool;
use std::error::Error;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct EmailSender {
    pool: PgPool,
    config: Config,
    #[allow(dead_code)]
    transport: AsyncSmtpTransport<lettre::Tokio1Executor>,
}

impl EmailSender {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let mut transport_builder = if let (Some(username), Some(password)) = 
            (&config.smtp.username, &config.smtp.password) {
            AsyncSmtpTransport::<lettre::Tokio1Executor>::relay(&config.smtp.hostname)
                .unwrap()
                .credentials(Credentials::new(username.clone(), password.clone()))
        } else {
            AsyncSmtpTransport::<lettre::Tokio1Executor>::relay(&config.smtp.hostname)
                .unwrap()
        };

        // Import the Tls enum from lettre
        use lettre::transport::smtp::client::Tls;
        
        transport_builder = transport_builder
            .port(config.smtp.port)
            .tls(Tls::Required(TlsParameters::new(config.smtp.hostname.clone()).unwrap()));

        Self {
            pool,
            config: config.clone(),
            transport: transport_builder.build(),
        }
    }

    pub async fn queue_email(
        &self,
        from_email: &str,
        to_email: &str,
        subject: &str,
        body_text: &str,
        body_html: Option<&str>,
    ) -> Result<Uuid, Box<dyn Error>> {
        // Validate that from_email is configured in one of our domains
        let domain = from_email.split('@').nth(1).ok_or("Invalid from email format")?;
        let domain_config = self.config.domains.iter()
            .find(|d| d.domain == domain)
            .ok_or(format!("Domain {} is not configured", domain))?;
        
        let is_valid_from = domain_config.from_addresses.iter()
            .any(|addr| addr == from_email || addr == "*");
        
        if !is_valid_from {
            return Err(format!("From address {} is not allowed for domain {}", from_email, domain).into());
        }
        
        // Check rate limits
        self.check_rate_limits(from_email, domain).await?;
        
        // Store email in database
        let email_id = db::store_email(
            &self.pool,
            from_email,
            to_email,
            subject,
            body_text,
            body_html,
        ).await?;
        
        info!("Email queued with ID: {}", email_id);
        Ok(email_id)
    }

    async fn check_rate_limits(&self, from_email: &str, domain: &str) -> Result<(), Box<dyn Error>> {
        // Check minute limit
        let minute_count = db::count_emails_for_rate_limit(&self.pool, from_email, domain, 1).await?;
        if minute_count >= self.config.rate_limits.emails_per_minute as i64 {
            return Err("Rate limit exceeded: too many emails per minute".into());
        }
        
        // Check hour limit
        let hour_count = db::count_emails_for_rate_limit(&self.pool, from_email, domain, 60).await?;
        if hour_count >= self.config.rate_limits.emails_per_hour as i64 {
            return Err("Rate limit exceeded: too many emails per hour".into());
        }
        
        // Check day limit
        let day_count = db::count_emails_for_rate_limit(&self.pool, from_email, domain, 1440).await?;
        if day_count >= self.config.rate_limits.emails_per_day as i64 {
            return Err("Rate limit exceeded: too many emails per day".into());
        }
        
        Ok(())
    }

    pub async fn process_email_queue(&self) -> Result<usize, Box<dyn Error>> {
        let batch_size = 50;
        let pending_emails = db::get_pending_emails(&self.pool, batch_size).await?;
        
        if pending_emails.is_empty() {
            return Ok(0);
        }
        
        info!("Processing {} emails from queue", pending_emails.len());
        
        let mut sent_count = 0;
        
        for email in pending_emails {
            match self.send_queued_email(&email).await {
                Ok(_) => {
                    db::update_email_status(&self.pool, email.id, EmailStatus::Sent, None).await?;
                    sent_count += 1;
                }
                Err(e) => {
                    let error_message = e.to_string();
                    warn!("Failed to send email {}: {}", email.id, error_message);
                    
                    let retry_count = email.retry_count as u32;
                    if retry_count < self.config.retries.max_retries {
                        let next_retry = calculate_next_retry(&self.config.retries, retry_count);
                        db::increment_retry_count(&self.pool, email.id, next_retry, Some(&error_message)).await?;
                    } else {
                        error!("Email {} has failed {} times, marking as failed", email.id, retry_count);
                        db::update_email_status(&self.pool, email.id, EmailStatus::Failed, Some(&error_message)).await?;
                    }
                }
            }
        }
        
        info!("Sent {} emails from queue", sent_count);
        Ok(sent_count)
    }

    async fn send_queued_email(&self, email: &db::EmailRecord) -> Result<(), Box<dyn Error>> {
        // Mark as sending
        db::update_email_status(&self.pool, email.id, EmailStatus::Sending, None).await?;
        
        // Parse sender and recipient
        let from = email.from_email.parse::<Mailbox>()?;
        let to = email.to_email.parse::<Mailbox>()?;
        
        // Build the email
        let builder = MessageBuilder::new()
            .from(from.clone())
            .to(to)
            .subject(&email.subject)
            .date_now();
        
        // Create the message based on content type
        let mut message: Message;
        
        // Set appropriate content type and body
        if let Some(html) = &email.body_html {
            message = builder
                .header(ContentType::TEXT_HTML)
                .body(html.clone())?;
        } else {
            message = builder
                .header(ContentType::TEXT_PLAIN)
                .body(email.body_text.clone())?;
        }
        
        // Add DKIM signature
        let domain = from.email.domain().to_string();
        let domain_config = self.config.domains.iter()
            .find(|d| d.domain == domain)
            .ok_or(format!("Domain {} is not configured", domain))?;
        
        // Sign the email with DKIM
        message = sign_email_with_dkim(&self.pool, &message, domain_config).await?;
        
        // Send the email
        let result = self.transport.send(message).await?;
        info!("Email {} sent: {:?}", email.id, result);
        
        Ok(())
    }
}

fn calculate_next_retry(retry_config: &RetryConfig, current_retry: u32) -> DateTime<Utc> {
    let now = Utc::now();
    let base_seconds = retry_config.initial_backoff_seconds as i64;
    
    // Exponential backoff: initial_backoff * 2^retry_count
    let backoff_seconds = std::cmp::min(
        base_seconds * 2i64.pow(current_retry),
        retry_config.max_backoff_seconds as i64
    );
    
    now + Duration::seconds(backoff_seconds)
}
