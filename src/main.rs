mod api;
mod auth;
mod config;
mod db;
mod email;
mod smtp_listener;

use api::setup_api;
use auth::generate_keys_if_needed;
use db::setup_database;
use smtp_listener::SmtpListenerService;
use std::env;
use std::net::SocketAddr;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting mailsender service");

    // Load configuration
    let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.yaml".to_string());
    let config = config::load_config(&config_path).expect("Failed to load configuration");
    
    info!("Configuration loaded: {} domains configured", config.domains.len());

    // Setup database
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = setup_database(&database_url).await?;
    
    // Generate DKIM keys if needed and output DNS settings
    generate_keys_if_needed(&pool, &config).await?;

    // Create email sender service
    let email_sender = email::EmailSender::new(pool.clone(), config.clone());
    
    // Set up SMTP listener (for port 25 direct email sending)
    let smtp_service = SmtpListenerService::new(config.clone(), pool.clone(), email_sender.clone());
    
    // Start API server and SMTP listener concurrently
    let api_port = config.api.port;
    let addr = SocketAddr::from(([0, 0, 0, 0], api_port));
    info!("API server listening on {}", addr);

    let app = setup_api(pool, config.clone(), email_sender);
    
    // Run services concurrently
    tokio::select! {
        // HTTP API service
        api_result = axum::Server::bind(&addr).serve(app.into_make_service()) => {
            if let Err(e) = api_result {
                error!("API server error: {}", e);
                return Err(e.into());
            }
        },
        
        // SMTP listener service
        smtp_result = smtp_service.start() => {
            if let Err(e) = smtp_result {
                error!("SMTP listener error: {}", e);
                return Err(e.into());
            }
        }
    };

    Ok(())
}
