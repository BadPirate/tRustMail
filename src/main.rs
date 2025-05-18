mod api;
mod auth;
mod config;
mod db;
mod email;

use api::setup_api;
use auth::generate_keys_if_needed;
use config::Config;
use db::setup_database;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{info, warn, Level};
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

    // Start API server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let listener = TcpListener::bind(addr).await?;
    info!("API server listening on {}", addr);

    let app = setup_api(pool, config);
    axum::Server::from_tcp(listener)?
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
