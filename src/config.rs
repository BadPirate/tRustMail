use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub domains: Vec<DomainConfig>,
    pub smtp: SmtpConfig,
    pub rate_limits: RateLimitConfig,
    pub retries: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConfig {
    pub domain: String,
    pub from_addresses: Vec<String>,
    pub dkim_selector: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub hostname: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub emails_per_minute: u32,
    pub emails_per_hour: u32,
    pub emails_per_day: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff_seconds: u32,
    pub max_backoff_seconds: u32,
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let config: Config = serde_yaml::from_str(&contents)?;
    
    // Validate config
    if config.domains.is_empty() {
        error!("No domains configured");
        return Err("At least one domain must be configured".into());
    }
    
    for domain in &config.domains {
        if domain.from_addresses.is_empty() {
            error!("Domain {} has no from_addresses configured", domain.domain);
            return Err(format!("Domain {} must have at least one from_address", domain.domain).into());
        }
    }
    
    info!("Configuration loaded successfully");
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_config() {
        let config_str = r#"
domains:
  - domain: example.com
    from_addresses:
      - noreply@example.com
      - support@example.com
    dkim_selector: default
smtp:
  hostname: smtp.example.com
  port: 25
  username: user
  password: pass
  max_connections: 5
rate_limits:
  emails_per_minute: 60
  emails_per_hour: 1000
  emails_per_day: 10000
retries:
  max_retries: 3
  initial_backoff_seconds: 60
  max_backoff_seconds: 3600
"#;
        
        let config: Config = serde_yaml::from_str(config_str).unwrap();
        assert_eq!(config.domains.len(), 1);
        assert_eq!(config.domains[0].domain, "example.com");
        assert_eq!(config.domains[0].from_addresses.len(), 2);
        assert_eq!(config.smtp.port, 25);
        assert_eq!(config.rate_limits.emails_per_minute, 60);
        assert_eq!(config.retries.max_retries, 3);
    }
}
