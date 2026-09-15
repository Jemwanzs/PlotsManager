//! Cross-tenant administration for the platform owner (one account, not a
//! tenant role — see `database/migrations/0004_platform_ownership.sql`).
//! Every handler here deliberately does NOT scope by the caller's own
//! `organization_id` the way every other route does: that's the entire
//! point of this module, so each one starts by checking
//! `AuthUser.is_platform_owner` instead.
//!
//! No frontend consumes this yet (see docs/14-development-roadmap.md) —
//! response shapes live here rather than in `domain` until a UI actually
//! needs to share them, per the project's established rule that `domain`
//! holds the *wire contract*, not speculative future consumers.

use axum::extract::Path;
use axum::{extract::State, routing::get, routing::post, Json, Router};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/platform/organizations", get(list_organizations))
        .route(
            "/api/v1/platform/organizations/:id",
            get(get_organization),
        )
        .route(
            "/api/v1/platform/organizations/:id/deactivate",
            post(deactivate_organization),
        )
        .route(
            "/api/v1/platform/organizations/:id/reactivate",
            post(reactivate_organization),
        )
}

fn require_platform_owner(auth: &AuthUser) -> Result<(), AppError> {
    if auth.is_platform_owner {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "This account doesn't have platform administrator access.",
        ))
    }
}

#[derive(Serialize, sqlx::FromRow)]
struct TenantSummary {
    id: Uuid,
    name: String,
    code: String,
    status: String,
    created_at: DateTime<Utc>,
    user_count: i64,
    subscription_status: Option<String>,
    trial_ends_at: Option<DateTime<Utc>>,
    plan_name: Option<String>,
}

const TENANT_SUMMARY_QUERY: &str = r#"
    select o.id, o.name, o.code, o.status, o.created_at,
        (select count(*) from users u where u.organization_id = o.id) as user_count,
        os.status as subscription_status,
        os.current_period_end as trial_ends_at,
        sp.name as plan_name
    from organizations o
    left join organization_subscriptions os on os.organization_id = o.id
    left join subscription_plans sp on sp.id = os.plan_id
"#;

async fn list_organizations(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<TenantSummary>>, AppError> {
    require_platform_owner(&auth)?;

    let rows: Vec<TenantSummary> = sqlx::query_as(&format!(
        "{TENANT_SUMMARY_QUERY} order by o.created_at"
    ))
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

#[derive(Serialize, sqlx::FromRow)]
struct TenantUser {
    id: Uuid,
    full_name: String,
    email: String,
    is_active: bool,
    is_platform_owner: bool,
    created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct AccessLogEntry {
    actor_id: Option<Uuid>,
    actor_name: Option<String>,
    action: String,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct TenantDetail {
    #[serde(flatten)]
    summary: TenantSummary,
    users: Vec<TenantUser>,
    recent_access: Vec<AccessLogEntry>,
}

async fn get_organization(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TenantDetail>, AppError> {
    require_platform_owner(&auth)?;

    let summary: Option<TenantSummary> =
        sqlx::query_as(&format!("{TENANT_SUMMARY_QUERY} where o.id = $1"))
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let summary = summary.ok_or(AppError::NotFound)?;

    let users: Vec<TenantUser> = sqlx::query_as(
        r#"select id, full_name, email, is_active, is_platform_owner, created_at
           from users where organization_id = $1 order by created_at"#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let recent_access: Vec<AccessLogEntry> = sqlx::query_as(
        r#"
        select a.actor_id, u.full_name as actor_name, a.action, a.created_at
        from audit_log a
        left join users u on u.id = a.actor_id
        where a.organization_id = $1 and a.entity_type = 'session'
        order by a.created_at desc
        limit 50
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(TenantDetail {
        summary,
        users,
        recent_access,
    }))
}

async fn deactivate_organization(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_platform_owner(&auth)?;
    set_organization_status(&state, id, "deactivated").await
}

async fn reactivate_organization(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_platform_owner(&auth)?;
    set_organization_status(&state, id, "active").await
}

async fn set_organization_status(
    state: &AppState,
    id: Uuid,
    status: &str,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = sqlx::query("update organizations set status = $1 where id = $2")
        .bind(status)
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({ "id": id, "status": status })))
}
