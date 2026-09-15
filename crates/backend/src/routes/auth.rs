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
    is_platform_owner: bool,
    created_at: DateTime<Utc>,
    // joined context, used only for the checks below
    org_status: String,
    subscription_status: Option<String>,
    trial_ends_at: Option<DateTime<Utc>>,
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
        r#"
        select u.id, u.organization_id, u.branch_id, u.full_name, u.email, u.password_hash,
            u.is_active, u.is_platform_owner, u.created_at,
            o.status as org_status,
            os.status as subscription_status, os.current_period_end as trial_ends_at
        from users u
        join organizations o on o.id = u.organization_id
        left join organization_subscriptions os on os.organization_id = o.id
        where u.email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(generic_error)?;

    if !row.is_active {
        return Err(AppError::Unauthorized);
    }

    if row.org_status == "deactivated" {
        return Err(AppError::forbidden(
            "This organization's account has been deactivated. Contact your platform administrator.",
        ));
    }

    // The platform owner's own organization is never trial-gated. A
    // tenant with no organization_subscriptions row at all (shouldn't
    // happen for anything provisioned after 0004, but true of pre-existing
    // dev/demo data) is likewise left unrestricted rather than locked out.
    if !row.is_platform_owner {
        if let (Some(status), Some(trial_ends_at)) =
            (row.subscription_status.as_deref(), row.trial_ends_at)
        {
            if status == "trialing" && trial_ends_at < Utc::now() {
                return Err(AppError::forbidden(
                    "Your trial period has expired. Contact us to continue using Real Estate Manager.",
                ));
            }
        }
    }

    let valid =
        verify_password(&input.password, &row.password_hash).map_err(|e| AppError::Internal(e.into()))?;
    if !valid {
        return Err(generic_error());
    }

    let token = issue_session_token(
        row.id,
        row.organization_id,
        row.is_platform_owner,
        &state.jwt_secret,
        Duration::hours(SESSION_TTL_HOURS),
    )
    .map_err(|e| AppError::Internal(e.into()))?;

    // Access history for the platform-admin view
    // (routes/platform.rs) — best-effort: a logging failure shouldn't
    // block a legitimate login.
    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'session', $2, 'login')"#,
    )
    .bind(row.organization_id)
    .bind(row.id)
    .execute(&state.db)
    .await;

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
        },
    }))
}
