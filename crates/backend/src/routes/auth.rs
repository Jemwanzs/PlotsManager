use axum::{extract::State, routing::post, Json, Router};
use chrono::{DateTime, Duration, Utc};
use domain::{AuthSession, LoginInput, SignupInput, User};
use uuid::Uuid;

use crate::auth::{hash_password, issue_session_token, verify_password};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/signup", post(signup))
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
    must_change_password: bool,
    temp_password_expires_at: Option<DateTime<Utc>>,
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
            u.must_change_password, u.temp_password_expires_at,
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

    crate::tenant_gate::check(
        &row.org_status,
        row.is_platform_owner,
        row.subscription_status.as_deref(),
        row.trial_ends_at,
    )?;

    let valid =
        verify_password(&input.password, &row.password_hash).map_err(|e| AppError::Internal(e.into()))?;
    if !valid {
        return Err(generic_error());
    }

    if row.must_change_password {
        if let Some(expires_at) = row.temp_password_expires_at {
            if expires_at < Utc::now() {
                return Err(AppError::bad_request(
                    "Your temporary password has expired. Ask an admin to reset it.",
                ));
            }
        }
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
    // (routes/platform.rs) and the Users & Access "Last Login" column —
    // best-effort: a logging failure shouldn't block a legitimate login.
    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'session', $2, 'login')"#,
    )
    .bind(row.organization_id)
    .bind(row.id)
    .execute(&state.db)
    .await;
    let _ = sqlx::query("update users set last_login_at = now() where id = $1")
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
            must_change_password: row.must_change_password,
        },
    }))
}

const TRIAL_HOURS: i64 = 48;

/// Creates a brand-new tenant — organization, first (admin) user, and a
/// 48-hour trial — in one transaction, then signs them straight in. See
/// docs/16-billing-and-subscriptions.md's "Org + first-admin sign-up
/// sequencing" note: nothing before this handler could create that
/// first row transactionally, which is exactly why platform-owner
/// bootstrap so far has been a manual one-off script, not this endpoint.
async fn signup(
    State(state): State<AppState>,
    Json(input): Json<SignupInput>,
) -> Result<Json<AuthSession>, AppError> {
    let org_name = input.organization_name.trim();
    let org_code = input.organization_code.trim().to_uppercase();
    let admin_name = input.admin_full_name.trim();
    let admin_email = input.admin_email.trim();

    if org_name.is_empty() || org_code.is_empty() {
        return Err(AppError::bad_request(
            "Enter an organization name and code.",
        ));
    }
    if admin_name.is_empty() || admin_email.is_empty() {
        return Err(AppError::bad_request("Enter your name and email."));
    }
    if input.admin_password.len() < 8 {
        return Err(AppError::bad_request(
            "Password must be at least 8 characters.",
        ));
    }

    let org_code_taken: bool =
        sqlx::query_scalar("select exists(select 1 from organizations where code = $1)")
            .bind(&org_code)
            .fetch_one(&state.db)
            .await?;
    if org_code_taken {
        return Err(AppError::conflict(format!(
            "Organization code \"{org_code}\" is already in use."
        )));
    }

    let email_taken: bool =
        sqlx::query_scalar("select exists(select 1 from users where email = $1)")
            .bind(admin_email)
            .fetch_one(&state.db)
            .await?;
    if email_taken {
        return Err(AppError::conflict(
            "An account with that email already exists.",
        ));
    }

    let password_hash =
        hash_password(&input.admin_password).map_err(|e| AppError::Internal(e.into()))?;

    let mut tx = state.db.begin().await?;

    let org_id: Uuid = sqlx::query_scalar(
        "insert into organizations (name, code) values ($1, $2) returning id",
    )
    .bind(org_name)
    .bind(&org_code)
    .fetch_one(&mut *tx)
    .await?;

    let role_id: Uuid = sqlx::query_scalar(
        r#"insert into roles (organization_id, name, permissions)
           values ($1, 'Admin', '["*"]'::jsonb) returning id"#,
    )
    .bind(org_id)
    .fetch_one(&mut *tx)
    .await?;

    let user_id: Uuid = sqlx::query_scalar(
        "insert into users (organization_id, full_name, email, password_hash) values ($1, $2, $3, $4) returning id",
    )
    .bind(org_id)
    .bind(admin_name)
    .bind(admin_email)
    .bind(&password_hash)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("insert into role_assignments (user_id, role_id) values ($1, $2)")
        .bind(user_id)
        .bind(role_id)
        .execute(&mut *tx)
        .await?;

    // The 'TRIAL' plan is reference data seeded by migration 0005 — it
    // exists in every environment, never just some.
    let trial_plan_id: Uuid = sqlx::query_scalar("select id from subscription_plans where code = 'TRIAL'")
        .fetch_one(&mut *tx)
        .await?;

    sqlx::query(
        &format!(
            r#"insert into organization_subscriptions (organization_id, plan_id, status, current_period_start, current_period_end)
               values ($1, $2, 'trialing', now(), now() + interval '{TRIAL_HOURS} hours')"#
        ),
    )
    .bind(org_id)
    .bind(trial_plan_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'session', $2, 'login')"#,
    )
    .bind(org_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    // Every organization gets a plot/project numbering config from the
    // moment it exists — matches the backfill migration 0010 runs for
    // orgs that predate it, so `GET /api/v1/settings` never has to
    // special-case "not configured yet".
    sqlx::query(
        r#"insert into numbering_sequences (organization_id, entity_type, prefix, padding)
           values ($1, 'plot', 'PLT', 4), ($1, 'project', 'PRJ', 4)"#,
    )
    .bind(org_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let token = issue_session_token(
        user_id,
        org_id,
        false,
        &state.jwt_secret,
        Duration::hours(SESSION_TTL_HOURS),
    )
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(AuthSession {
        token,
        user: User {
            id: user_id,
            organization_id: org_id,
            branch_id: None,
            full_name: admin_name.to_string(),
            email: admin_email.to_string(),
            is_active: true,
            is_platform_owner: false,
            created_at: Utc::now(),
            must_change_password: false,
        },
    }))
}
