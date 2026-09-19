//! Profile -> Security -> Change Password. The authenticated user
//! changes their own password — distinct from `routes/users.rs`'s
//! admin-initiated reset (different actor, no `PERM_MANAGE_USERS`
//! check, just proof of the current password). Issues a fresh session
//! token in the response: bumping `session_valid_after` as part of the
//! change (same defense-in-depth every password reset gets) would
//! otherwise invalidate the very session that just made this request.

use axum::{extract::State, routing::put, Json, Router};
use chrono::Utc;
use domain::{AuthSession, ChangePasswordInput, User};
use uuid::Uuid;

use crate::auth::{hash_password, issue_session_token, verify_password};
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

const SESSION_TTL_HOURS: i64 = 24;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/account/change-password", put(change_password))
}

#[derive(sqlx::FromRow)]
struct SelfRow {
    id: Uuid,
    organization_id: Uuid,
    branch_id: Option<Uuid>,
    full_name: String,
    email: String,
    password_hash: String,
    is_active: bool,
    is_platform_owner: bool,
    created_at: chrono::DateTime<Utc>,
}

async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<ChangePasswordInput>,
) -> Result<Json<AuthSession>, AppError> {
    if input.new_password.len() < 8 {
        return Err(AppError::bad_request(
            "New password must be at least 8 characters.",
        ));
    }

    let row: SelfRow = sqlx::query_as(
        r#"select id, organization_id, branch_id, full_name, email, password_hash,
               is_active, is_platform_owner, created_at
           from users where id = $1"#,
    )
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await?;

    let valid = verify_password(&input.current_password, &row.password_hash)
        .map_err(|e| AppError::Internal(e.into()))?;
    if !valid {
        return Err(AppError::bad_request("Your current password is incorrect."));
    }

    let new_hash =
        hash_password(&input.new_password).map_err(|e| AppError::Internal(e.into()))?;

    sqlx::query(
        r#"update users set
               password_hash = $1,
               must_change_password = false,
               temp_password_expires_at = null,
               password_changed_at = now(),
               session_valid_after = now()
           where id = $2"#,
    )
    .bind(&new_hash)
    .bind(auth.user_id)
    .execute(&state.db)
    .await?;

    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'user', $2, 'password_changed')"#,
    )
    .bind(auth.organization_id)
    .bind(auth.user_id)
    .execute(&state.db)
    .await;

    // The update above just invalidated every token issued before now,
    // including the one that authenticated this very request — issue a
    // fresh one so the caller isn't immediately signed out.
    let token = issue_session_token(
        row.id,
        row.organization_id,
        row.is_platform_owner,
        &state.jwt_secret,
        chrono::Duration::hours(SESSION_TTL_HOURS),
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
            is_platform_owner: row.is_platform_owner,
            created_at: row.created_at,
            must_change_password: false,
        },
    }))
}
