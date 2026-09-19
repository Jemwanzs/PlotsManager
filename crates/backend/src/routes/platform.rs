//! Cross-tenant administration for the platform owner (one account, not a
//! tenant role — see `database/migrations/0004_platform_ownership.sql`).
//! Every handler here deliberately does NOT scope by the caller's own
//! `organization_id` the way every other route does: that's the entire
//! point of this module, so each one starts by checking
//! `AuthUser.is_platform_owner` instead.

use axum::extract::Path;
use axum::{extract::State, routing::get, routing::post, Json, Router};
use chrono::{DateTime, Duration, Utc};
use domain::{
    PlatformAccessLogEntry, PlatformOrganizationDetail, PlatformOrganizationSummary,
    PlatformOrganizationUser, RejectOrganizationInput,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

/// Phase-1 fixed trial length for a newly approved tenant — the
/// tenant-onboarding spec calls for this to be Platform-Owner-
/// configurable, which is a later phase of the same spec; this is the
/// default that phase will make adjustable, not a final answer.
const DEFAULT_TRIAL_DAYS: i64 = 14;

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
        .route(
            "/api/v1/platform/organizations/:id/approve",
            post(approve_organization),
        )
        .route(
            "/api/v1/platform/organizations/:id/reject",
            post(reject_organization),
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

#[derive(sqlx::FromRow)]
struct TenantSummaryRow {
    id: Uuid,
    name: String,
    code: String,
    status: String,
    created_at: DateTime<Utc>,
    user_count: i64,
    subscription_status: Option<String>,
    trial_ends_at: Option<DateTime<Utc>>,
    plan_name: Option<String>,
    business_registration_number: Option<String>,
    sector: Option<String>,
    business_location: Option<String>,
    contact_person_name: Option<String>,
    expected_users: Option<i32>,
    number_of_branches: Option<i32>,
    preferred_package_code: Option<String>,
    approved_at: Option<DateTime<Utc>>,
    approved_by_name: Option<String>,
    rejected_at: Option<DateTime<Utc>>,
    rejected_reason: Option<String>,
}

impl From<TenantSummaryRow> for PlatformOrganizationSummary {
    fn from(r: TenantSummaryRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            code: r.code,
            status: r.status,
            created_at: r.created_at,
            user_count: r.user_count,
            subscription_status: r.subscription_status,
            trial_ends_at: r.trial_ends_at,
            plan_name: r.plan_name,
            business_registration_number: r.business_registration_number,
            sector: r.sector,
            business_location: r.business_location,
            contact_person_name: r.contact_person_name,
            expected_users: r.expected_users,
            number_of_branches: r.number_of_branches,
            preferred_package_code: r.preferred_package_code,
            approved_at: r.approved_at,
            approved_by_name: r.approved_by_name,
            rejected_at: r.rejected_at,
            rejected_reason: r.rejected_reason,
        }
    }
}

const TENANT_SUMMARY_QUERY: &str = r#"
    select o.id, o.name, o.code, o.status, o.created_at,
        (select count(*) from users u where u.organization_id = o.id) as user_count,
        os.status as subscription_status,
        os.current_period_end as trial_ends_at,
        sp.name as plan_name,
        o.business_registration_number, o.sector, o.business_location,
        o.contact_person_name, o.expected_users, o.number_of_branches,
        o.preferred_package_code, o.approved_at, approver.full_name as approved_by_name,
        o.rejected_at, o.rejected_reason
    from organizations o
    left join organization_subscriptions os on os.organization_id = o.id
    left join subscription_plans sp on sp.id = os.plan_id
    left join users approver on approver.id = o.approved_by
"#;

async fn list_organizations(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<PlatformOrganizationSummary>>, AppError> {
    require_platform_owner(&auth)?;

    let rows: Vec<TenantSummaryRow> = sqlx::query_as(&format!(
        "{TENANT_SUMMARY_QUERY} order by o.created_at"
    ))
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

#[derive(sqlx::FromRow)]
struct TenantUserRow {
    id: Uuid,
    full_name: String,
    email: String,
    is_active: bool,
    is_platform_owner: bool,
    created_at: DateTime<Utc>,
}

impl From<TenantUserRow> for PlatformOrganizationUser {
    fn from(r: TenantUserRow) -> Self {
        Self {
            id: r.id,
            full_name: r.full_name,
            email: r.email,
            is_active: r.is_active,
            is_platform_owner: r.is_platform_owner,
            created_at: r.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct AccessLogRow {
    actor_id: Option<Uuid>,
    actor_name: Option<String>,
    action: String,
    created_at: DateTime<Utc>,
}

impl From<AccessLogRow> for PlatformAccessLogEntry {
    fn from(r: AccessLogRow) -> Self {
        Self {
            actor_id: r.actor_id,
            actor_name: r.actor_name,
            action: r.action,
            created_at: r.created_at,
        }
    }
}

async fn get_organization(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PlatformOrganizationDetail>, AppError> {
    require_platform_owner(&auth)?;

    let summary: Option<TenantSummaryRow> =
        sqlx::query_as(&format!("{TENANT_SUMMARY_QUERY} where o.id = $1"))
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let summary: PlatformOrganizationSummary = summary.ok_or(AppError::NotFound)?.into();

    let user_rows: Vec<TenantUserRow> = sqlx::query_as(
        r#"select id, full_name, email, is_active, is_platform_owner, created_at
           from users where organization_id = $1 order by created_at"#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let access_rows: Vec<AccessLogRow> = sqlx::query_as(
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

    Ok(Json(PlatformOrganizationDetail {
        summary,
        users: user_rows.into_iter().map(Into::into).collect(),
        recent_access: access_rows.into_iter().map(Into::into).collect(),
    }))
}

async fn deactivate_organization(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_platform_owner(&auth)?;
    // The platform owner's own organization is exempt from the
    // deactivated-status check itself (`tenant_gate::check`), but never
    // reaching that state in the first place is the clearer fix: a
    // deactivated status on this org would read as "this tenant is
    // suspended" everywhere else in the product (the org list, the org
    // detail page's own badge) even though it can never actually lock
    // this account out.
    if id == auth.organization_id {
        return Err(AppError::bad_request(
            "You can't deactivate the platform owner's own organization.",
        ));
    }
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

/// "Approve & Start Trial" — the trial clock starts now, not at the
/// original sign-up time, so an applicant waiting on review never
/// loses trial days to that wait (the whole point of gating access at
/// sign-up in the first place).
async fn approve_organization(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PlatformOrganizationSummary>, AppError> {
    require_platform_owner(&auth)?;

    let current_status: Option<String> =
        sqlx::query_scalar("select status from organizations where id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let current_status = current_status.ok_or(AppError::NotFound)?;
    if current_status != "pending_approval" {
        return Err(AppError::conflict(format!(
            "This tenant is \"{current_status}\", not awaiting approval."
        )));
    }

    let mut tx = state.db.begin().await?;

    sqlx::query(
        "update organizations set status = 'trial_active', approved_at = now(), approved_by = $1 where id = $2",
    )
    .bind(auth.user_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    let trial_plan_id: Uuid =
        sqlx::query_scalar("select id from subscription_plans where code = 'TRIAL'")
            .fetch_one(&mut *tx)
            .await?;
    let trial_end = Utc::now() + Duration::days(DEFAULT_TRIAL_DAYS);
    sqlx::query(
        r#"insert into organization_subscriptions (organization_id, plan_id, status, current_period_start, current_period_end)
           values ($1, $2, 'trialing', now(), $3)
           on conflict (organization_id) do update set
               plan_id = excluded.plan_id, status = excluded.status,
               current_period_start = excluded.current_period_start,
               current_period_end = excluded.current_period_end, updated_at = now()"#,
    )
    .bind(id)
    .bind(trial_plan_id)
    .bind(trial_end)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'organization', $1, 'tenant_approved')"#,
    )
    .bind(id)
    .bind(auth.user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let row: TenantSummaryRow = sqlx::query_as(&format!("{TENANT_SUMMARY_QUERY} where o.id = $1"))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(row.into()))
}

async fn reject_organization(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<RejectOrganizationInput>,
) -> Result<Json<PlatformOrganizationSummary>, AppError> {
    require_platform_owner(&auth)?;

    let reason = input.reason.trim();
    if reason.is_empty() {
        return Err(AppError::bad_request("Enter a reason for rejecting this application."));
    }

    let current_status: Option<String> =
        sqlx::query_scalar("select status from organizations where id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let current_status = current_status.ok_or(AppError::NotFound)?;
    if current_status != "pending_approval" {
        return Err(AppError::conflict(format!(
            "This tenant is \"{current_status}\", not awaiting approval."
        )));
    }

    sqlx::query(
        "update organizations set status = 'rejected', rejected_at = now(), rejected_reason = $1 where id = $2",
    )
    .bind(reason)
    .bind(id)
    .execute(&state.db)
    .await?;

    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action, after_state)
           values ($1, $2, 'organization', $1, 'tenant_rejected', $3)"#,
    )
    .bind(id)
    .bind(auth.user_id)
    .bind(serde_json::json!({ "reason": reason }))
    .execute(&state.db)
    .await;

    let row: TenantSummaryRow = sqlx::query_as(&format!("{TENANT_SUMMARY_QUERY} where o.id = $1"))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(row.into()))
}
