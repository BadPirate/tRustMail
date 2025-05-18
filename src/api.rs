use crate::config::Config;
use crate::email::EmailSender;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

// API types
#[derive(Debug, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendEmailResponse {
    pub id: Uuid,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub version: String,
}

// Application state
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub email_sender: EmailSender,
}

// Setup API routes
pub fn setup_api(pool: PgPool, config: Config) -> Router {
    let email_sender = EmailSender::new(pool.clone(), config.clone());
    let state = Arc::new(AppState {
        pool,
        config,
        email_sender,
    });

    Router::new()
        .route("/api/email", post(send_email))
        .route("/api/email/:id", get(get_email_status))
        .route("/api/health", get(health_check))
        .with_state(state)
}

// API handlers
async fn send_email(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SendEmailRequest>,
) -> Result<Json<SendEmailResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Received request to send email from: {} to: {}", request.from, request.to);

    match state.email_sender.queue_email(
        &request.from,
        &request.to,
        &request.subject,
        &request.text,
        request.html.as_deref(),
    ).await {
        Ok(id) => {
            let response = SendEmailResponse {
                id,
                status: "queued".to_string(),
            };
            Ok(Json(response))
        },
        Err(e) => {
            error!("Failed to queue email: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ))
        }
    }
}

async fn get_email_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    info!("Checking status for email: {}", id);

    // Query the email from the database
    let email = sqlx::query!(
        r#"
        SELECT 
            id, 
            status, 
            created_at, 
            updated_at, 
            sent_at, 
            retry_count, 
            next_retry_at, 
            error_message
        FROM emails
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Database error".to_string(),
            }),
        )
    })?;

    match email {
        Some(email) => {
            let result = serde_json::json!({
                "id": email.id,
                "status": email.status,
                "created_at": email.created_at,
                "updated_at": email.updated_at,
                "sent_at": email.sent_at,
                "retry_count": email.retry_count,
                "next_retry_at": email.next_retry_at,
                "error_message": email.error_message,
            });
            Ok(Json(result))
        },
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Email with ID {} not found", id),
            }),
        )),
    }
}

async fn health_check() -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
