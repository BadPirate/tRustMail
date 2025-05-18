use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailRecord {
    pub id: Uuid,
    pub from_email: String,
    pub to_email: String,
    pub subject: String,
    pub body_html: Option<String>,
    pub body_text: String,
    pub status: EmailStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "email_status", rename_all = "snake_case")]
pub enum EmailStatus {
    Pending,
    Sending,
    Sent,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomainKey {
    pub id: Uuid,
    pub domain: String,
    pub selector: String,
    pub private_key: String,
    pub public_key: String,
    pub created_at: DateTime<Utc>,
}

pub async fn setup_database(database_url: &str) -> Result<PgPool, sqlx::Error> {
    // Create connection pool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    
    // Run migrations
    info!("Running database migrations");
    sqlx::migrate!("./src/migrations")
        .run(&pool)
        .await?;
    
    info!("Database setup complete");
    Ok(pool)
}

pub async fn store_email(
    pool: &PgPool,
    from_email: &str,
    to_email: &str,
    subject: &str,
    body_text: &str,
    body_html: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    
    let result = sqlx::query!(
        r#"
        INSERT INTO emails (
            id, from_email, to_email, subject, body_text, body_html,
            status, created_at, updated_at, retry_count
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7, $8, 0)
        RETURNING id
        "#,
        id,
        from_email,
        to_email,
        subject,
        body_text,
        body_html,
        now,
        now
    )
    .fetch_one(pool)
    .await?;
    
    Ok(result.id)
}

pub async fn update_email_status(
    pool: &PgPool,
    email_id: Uuid,
    status: EmailStatus,
    error_message: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    
    let mut sent_at = None;
    if matches!(status, EmailStatus::Sent) {
        sent_at = Some(now);
    }
    
    sqlx::query!(
        r#"
        UPDATE emails
        SET status = $1, updated_at = $2, sent_at = $3, error_message = $4
        WHERE id = $5
        "#,
        status as EmailStatus,
        now,
        sent_at,
        error_message,
        email_id
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

pub async fn get_pending_emails(pool: &PgPool, limit: i64) -> Result<Vec<EmailRecord>, sqlx::Error> {
    let now = Utc::now();
    
    let records = sqlx::query_as!(
        EmailRecord,
        r#"
        SELECT 
            id, from_email, to_email, subject, body_html, body_text,
            status as "status: EmailStatus", created_at, updated_at, sent_at,
            retry_count, next_retry_at, error_message
        FROM emails
        WHERE status = 'pending' AND (next_retry_at IS NULL OR next_retry_at <= $1)
        ORDER BY created_at ASC
        LIMIT $2
        "#,
        now,
        limit
    )
    .fetch_all(pool)
    .await?;
    
    Ok(records)
}

pub async fn increment_retry_count(
    pool: &PgPool,
    email_id: Uuid,
    next_retry_at: DateTime<Utc>,
    error_message: Option<&str>,
) -> Result<i32, sqlx::Error> {
    let now = Utc::now();
    
    let result = sqlx::query!(
        r#"
        UPDATE emails
        SET retry_count = retry_count + 1,
            next_retry_at = $1,
            updated_at = $2,
            error_message = $3
        WHERE id = $4
        RETURNING retry_count
        "#,
        next_retry_at,
        now,
        error_message,
        email_id
    )
    .fetch_one(pool)
    .await?;
    
    Ok(result.retry_count)
}

pub async fn store_domain_key(
    pool: &PgPool,
    domain: &str,
    selector: &str,
    private_key: &str,
    public_key: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    
    let result = sqlx::query!(
        r#"
        INSERT INTO domain_keys (id, domain, selector, private_key, public_key, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
        id,
        domain,
        selector,
        private_key,
        public_key,
        now
    )
    .fetch_one(pool)
    .await?;
    
    Ok(result.id)
}

pub async fn get_domain_key(pool: &PgPool, domain: &str, selector: &str) -> Result<Option<DomainKey>, sqlx::Error> {
    let record = sqlx::query_as!(
        DomainKey,
        r#"
        SELECT id, domain, selector, private_key, public_key, created_at
        FROM domain_keys
        WHERE domain = $1 AND selector = $2
        "#,
        domain,
        selector
    )
    .fetch_optional(pool)
    .await?;
    
    Ok(record)
}

pub async fn count_emails_for_rate_limit(
    pool: &PgPool,
    from_email: &str,
    domain: &str,
    period_minutes: i64,
) -> Result<i64, sqlx::Error> {
    let now = Utc::now();
    let period_start = now - chrono::Duration::minutes(period_minutes);
    
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM emails
        WHERE (from_email = $1 OR from_email LIKE $2)
        AND created_at >= $3
        "#,
        from_email,
        format!("%@{}", domain),
        period_start
    )
    .fetch_one(pool)
    .await?;
    
    Ok(result.count.unwrap_or(0))
}
