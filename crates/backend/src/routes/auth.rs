use axum::{extract::State, routing::post, Json, Router};
use chrono::{DateTime, Duration, Utc};
use domain::{AuthSession, LoginInput, User};
use uuid::Uuid;

use crate::auth::{issue_session_token, verify_password};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/auth/login", post(login))
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    organization_id: Uuid,
    branch_id: Option<Uuid>,
    full_name: String,
    email: String,
    password_hash: String,
    is_active: bool,
    created_at: DateTime<Utc>,
}

const SESSION_TTL_HOURS: i64 = 24;

async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginInput>,
) -> Result<Json<AuthSession>, AppError> {
    let email = input.email.trim();
    let generic_error = || {
        AppError::bad_request("That email/password combination doesn't match our records.")
    };

    let row: Option<UserRow> = sqlx::query_as(
        r#"select id, organization_id, branch_id, full_name, email, password_hash, is_active, created_at
           from users where email = $1"#,
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(generic_error)?;

    if !row.is_active {
        return Err(AppError::Unauthorized);
    }

    let valid =
        verify_password(&input.password, &row.password_hash).map_err(|e| AppError::Internal(e.into()))?;
    if !valid {
        return Err(generic_error());
    }

    let token = issue_session_token(
        row.id,
        row.organization_id,
        &state.jwt_secret,
        Duration::hours(SESSION_TTL_HOURS),
    )
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(AuthSession {
        token,
        user: User {
            id: row.id,
            organization_id: row.organization_id,
            branch_id: row.branch_id,
            full_name: row.full_name,
            email: row.email,
            is_active: row.is_active,
            created_at: row.created_at,
        },
    }))
}
