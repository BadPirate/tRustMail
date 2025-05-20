use crate::config::{Config, SmtpListenerConfig};
use crate::email::EmailSender;
use mailin::{Handler as SmtpHandler, Response};
use sqlx::PgPool;
use std::io::Error as IoError;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

// Main SMTP service that will listen on port 25 (or configured port)
pub struct SmtpListenerService {
    config: SmtpListenerConfig,
    pool: PgPool,
    email_sender: EmailSender,
    api_keys: Vec<String>,
}

impl SmtpListenerService {
    pub fn new(config: Config, pool: PgPool, email_sender: EmailSender) -> Self {
        // Extract API keys for authentication
        let api_keys = config.api.api_keys.iter()
            .map(|key_config| key_config.key.clone())
            .collect();
            
        Self {
            config: config.smtp_listener,
            pool,
            email_sender,
            api_keys,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.config.enabled {
            info!("SMTP Listener is disabled in configuration");
            return Ok(());
        }

        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.port));
        let pool = self.pool.clone();
        let email_sender = self.email_sender.clone();
        let api_keys = self.api_keys.clone();
        let require_auth = self.config.require_auth;
        
        // Create channel for email delivery
        let (tx, mut rx) = mpsc::channel::<EmailTask>(32);
        
        // Spawn a handler for processing emails in the background
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        
        let email_processor: JoinHandle<()> = tokio::spawn(async move {
            while let Some(task) = rx.recv().await {
                match email_sender.queue_email(
                    &task.from_email,
                    &task.to_email,
                    &task.subject,
                    &task.body_text,
                    task.body_html.as_deref(),
                ).await {
                    Ok(id) => {
                        info!("Email queued from SMTP listener: {} -> {}: {} (ID: {})", 
                             task.from_email, task.to_email, task.subject, id);
                    },
                    Err(e) => {
                        error!("Failed to queue email from SMTP listener: {}", e);
                    }
                }
            }
            
            info!("Email processor for SMTP listener shut down");
        });
        
        // Setup SMTP server
        info!("SMTP Listener starting on port {}", self.config.port);
        
        // Create SMTP handler with dependencies
        let handler = MailinSmtpHandler {
            require_auth,
            api_keys,
            db_pool: pool,
            email_tx: tx,
            authenticated_clients: Arc::new(parking_lot::RwLock::new(Vec::new())),
        };
        
        // Create and start the SMTP server
        let server_handle = tokio::spawn(async move {
            info!("Starting SMTP server on {}", addr);
            
            // Create a TCP listener for handling SMTP connections
            match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => {
                    info!("SMTP server listening on {}", addr);
                    
                    // Accept connections and process them
                    loop {
                        match listener.accept().await {
                            Ok((socket, client_addr)) => {
                                info!("New SMTP connection from {}", client_addr);
                                
                                // Clone what we need for the connection handler
                                let mut handler_clone = handler.clone();
                                
                                // Spawn a task to handle this client
                                tokio::spawn(async move {
                                    if let Err(e) = process_smtp_client(socket, &mut handler_clone).await {
                                        error!("Error handling SMTP client {}: {}", client_addr, e);
                                    }
                                });
                            },
                            Err(e) => {
                                error!("Failed to accept SMTP connection: {}", e);
                                
                                // Short sleep to avoid tight loop on repeated errors
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            }
                        }
                        
                        // Check if we should exit
                        if !r.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                    
                    info!("SMTP server shutting down");
                },
                Err(e) => {
                    error!("Failed to bind SMTP server to {}: {}", addr, e);
                    r.store(false, Ordering::SeqCst);
                }
            }
        });
        
        // Return the handle
        Ok(())
    }
}

// Task for sending an email
struct EmailTask {
    from_email: String,
    to_email: String,
    subject: String,
    body_text: String,
    body_html: Option<String>,
}

// SMTP request handler implementation
#[derive(Clone)]
struct MailinSmtpHandler {
    require_auth: bool,
    api_keys: Vec<String>,
    db_pool: PgPool,
    email_tx: mpsc::Sender<EmailTask>,
    authenticated_clients: Arc<parking_lot::RwLock<Vec<IpAddr>>>,
}

impl SmtpHandler for MailinSmtpHandler {
    // Handle HELO command
    fn helo(&mut self, ip: IpAddr, domain: &str) -> Response {
        info!("SMTP HELO from {}:{}", ip, domain);
        Response::custom(250, "OK".to_string())
    }
    
    // Handle MAIL FROM command
    fn mail(&mut self, ip: IpAddr, from: &str, _extension: &str) -> Response {
        // Check authentication if required
        if self.require_auth {
            let authed = {
                let clients = self.authenticated_clients.read();
                clients.contains(&ip)
            };
            
            if !authed {
                return Response::custom(530, "Authentication required".to_string());
            }
        }
        
        // Basic email validation
        if !is_valid_email(from) {
            return Response::custom(550, "Invalid sender address".to_string());
        }
        
        info!("SMTP MAIL FROM accepted: {}", from);
        Response::custom(250, "OK".to_string())
    }

    // Handle RCPT TO command
    fn rcpt(&mut self, to: &str) -> Response {
        // Basic email validation
        if !is_valid_email(to) {
            return Response::custom(550, "Invalid recipient address".to_string());
        }
        
        info!("SMTP RCPT TO accepted: {}", to);
        Response::custom(250, "OK".to_string())
    }
    
    // Process email data
    fn data(&mut self, email: &[u8]) -> Result<(), IoError> {
        // Parse email data
        let email_str = match std::str::from_utf8(email) {
            Ok(s) => s,
            Err(_) => {
                error!("Email data is not valid UTF-8");
                return Err(IoError::new(
                    std::io::ErrorKind::InvalidData,
                    "Email data is not valid UTF-8"
                ));
            }
        };
        
        // Extract email parts (simplified parsing)
        let from = match extract_from(email_str) {
            Some(from) => from,
            None => {
                error!("Failed to extract email sender");
                return Err(IoError::new(
                    std::io::ErrorKind::InvalidData,
                    "Failed to extract email sender"
                ));
            }
        };
        
        let to = match extract_to(email_str) {
            Some(to) => to,
            None => {
                error!("Failed to extract email recipient");
                return Err(IoError::new(
                    std::io::ErrorKind::InvalidData,
                    "Failed to extract email recipient"
                ));
            }
        };
        
        let subject = extract_subject(email_str)
            .unwrap_or_else(|| "No Subject".to_string());
        
        let body = extract_body(email_str);
        
        // Create email task for background processing
        let task = EmailTask {
            from_email: from,
            to_email: to,
            subject: subject.clone(),
            body_text: body,
            body_html: None,
        };
        
        // Send the task to the processor
        if let Err(e) = self.email_tx.try_send(task) {
            error!("Failed to queue email task: {}", e);
            return Err(IoError::new(
                std::io::ErrorKind::Other,
                format!("Failed to queue email: {}", e)
            ));
        }
        
        Ok(())
    }
}

// Function to process an SMTP client connection
async fn process_smtp_client(
    socket: tokio::net::TcpStream,
    handler: &mut MailinSmtpHandler,
) -> Result<(), IoError> {
    // In a real implementation, this would handle the full SMTP protocol
    // For now, we'll just log the connection and return OK
    info!("Handling SMTP client connection");
    
    // Here would be the full implementation of the SMTP protocol
    // We would read commands from the socket, parse them, and call
    // the appropriate handler methods
    
    // For the MVP, we'll just return success
    Ok(())
}

// Helper functions
fn is_valid_email(email: &str) -> bool {
    // Simple validation - contains @ and no spaces
    // In production, you should use a proper email validation library/regex
    email.contains('@') && !email.contains(' ')
}

fn extract_from(data: &str) -> Option<String> {
    for line in data.lines() {
        if line.to_lowercase().starts_with("from:") {
            // Extract email address from the From header
            // This is a very simplified implementation
            let full_from = line[5..].trim();
            
            // Try to extract just the email part
            if let Some(start) = full_from.find('<') {
                if let Some(end) = full_from.find('>') {
                    if start < end {
                        return Some(full_from[start+1..end].to_string());
                    }
                }
            }
            
            // If no angle brackets, use the whole value
            return Some(full_from.to_string());
        }
    }
    None
}

fn extract_to(data: &str) -> Option<String> {
    for line in data.lines() {
        if line.to_lowercase().starts_with("to:") {
            // Extract email address from the To header
            // This is a very simplified implementation
            let full_to = line[3..].trim();
            
            // Try to extract just the email part
            if let Some(start) = full_to.find('<') {
                if let Some(end) = full_to.find('>') {
                    if start < end {
                        return Some(full_to[start+1..end].to_string());
                    }
                }
            }
            
            // If no angle brackets, use the whole value
            return Some(full_to.to_string());
        }
    }
    None
}

fn extract_subject(data: &str) -> Option<String> {
    for line in data.lines() {
        if line.to_lowercase().starts_with("subject:") {
            return Some(line[8..].trim().to_string());
        }
    }
    None
}

fn extract_body(data: &str) -> String {
    let mut in_body = false;
    let mut body_lines = Vec::new();
    
    for line in data.lines() {
        if in_body {
            body_lines.push(line);
        } else if line.trim().is_empty() {
            in_body = true;
        }
    }
    
    body_lines.join("\n")
}