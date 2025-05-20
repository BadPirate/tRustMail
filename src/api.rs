use crate::config::Config;
use crate::db;
use crate::email::EmailSender;
use axum::{
    extract::{Path, State},
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::Response,
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

// Authentication middleware
async fn auth_middleware<B>(
    State(state): State<Arc<AppState>>,
    req: Request<B>, 
    next: Next<B>
) -> Result<Response, StatusCode> {
    // Skip auth for health endpoint
    if req.uri().path().contains("/health") {
        return Ok(next.run(req).await);
    }
    
    // Check for API key in header
    let auth_header = req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());
    
    let api_key = match auth_header {
        Some(header) => {
            // Format should be "Bearer YOUR_API_KEY"
            let parts: Vec<&str> = header.split_whitespace().collect();
            if parts.len() == 2 && parts[0] == "Bearer" {
                parts[1]
            } else {
                return Err(StatusCode::UNAUTHORIZED);
            }
        },
        None => return Err(StatusCode::UNAUTHORIZED),
    };
    
    // Validate the API key
    let is_valid = state.config.api.api_keys.iter()
        .any(|key_config| key_config.key == api_key);
    
    if is_valid {
        Ok(next.run(req).await)
    } else {
        error!("Invalid API key provided");
        Err(StatusCode::UNAUTHORIZED)
    }
}

// Setup API routes
pub fn setup_api(pool: PgPool, config: Config, email_sender: EmailSender) -> Router {
    let state = Arc::new(AppState {
        pool,
        config,
        email_sender,
    });

    Router::new()
        .route("/api/email", post(send_email))
        .route("/api/email/:id", get(get_email_status))
        .route("/api/health", get(health_check))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
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
    let email = sqlx::query_as!(
        db::EmailRecord,
        r#"
        SELECT 
            id, 
            from_email,
            to_email,
            subject,
            body_text,
            body_html,
            status as "status: db::EmailStatus", 
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
