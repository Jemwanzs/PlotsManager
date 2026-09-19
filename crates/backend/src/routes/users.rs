//! Settings -> Users & Access. Tenant Admins manage the users belonging
//! to their own organization — never across the tenant boundary (every
//! query here is scoped to `auth.organization_id`, same as the rest of
//! the app): list, create, edit (name/email/mobile/branch/role),
//! activate/deactivate, reset a user's password, and revoke their
//! sessions. Self-service password change lives in
//! `crates/backend/src/routes/account.rs` instead — a different actor
//! (the user themselves, not an admin) and a different permission
//! model (no `PERM_MANAGE_USERS` check, just "is this you").

use axum::extract::Path;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Duration, Utc};
use domain::{
    CreateUserInput, ResetPasswordInput, TenantUser, UpdateUserInput, PERM_MANAGE_SESSIONS,
    PERM_MANAGE_USERS, PERM_RESET_PASSWORD,
};
use std::collections::HashSet;
use uuid::Uuid;

use crate::auth::hash_password;
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

/// How long an admin-issued temporary password stays valid before
/// login starts rejecting it (`routes/auth.rs`'s `login` handler) —
/// long enough that a new hire starting next week isn't already locked
/// out, short enough that a forgotten invite doesn't sit valid forever.
const TEMP_PASSWORD_TTL_DAYS: i64 = 7;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/users", get(list_users).post(create_user))
        .route("/api/v1/users/:id", axum::routing::put(update_user))
        .route("/api/v1/users/:id/activate", axum::routing::put(activate_user))
        .route("/api/v1/users/:id/deactivate", axum::routing::put(deactivate_user))
        .route("/api/v1/users/:id/reset-password", axum::routing::put(reset_password))
        .route("/api/v1/users/:id/revoke-sessions", axum::routing::put(revoke_sessions))
}

#[derive(sqlx::FromRow)]
struct TenantUserRow {
    id: Uuid,
    full_name: String,
    email: String,
    mobile: Option<String>,
    is_active: bool,
    branch_id: Option<Uuid>,
    branch_name: Option<String>,
    branch_ids: Vec<Uuid>,
    branch_count: i64,
    role_id: Option<Uuid>,
    role_name: Option<String>,
    last_login_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    must_change_password: bool,
    password_changed_at: DateTime<Utc>,
}

impl TenantUserRow {
    fn into_domain(self) -> TenantUser {
        TenantUser {
            id: self.id,
            full_name: self.full_name,
            email: self.email,
            mobile: self.mobile,
            is_active: self.is_active,
            branch_id: self.branch_id,
            branch_name: self.branch_name,
            branch_ids: self.branch_ids,
            branch_count: self.branch_count,
            role_id: self.role_id,
            role_name: self.role_name,
            last_login_at: self.last_login_at,
            created_at: self.created_at,
            must_change_password: self.must_change_password,
            password_changed_at: self.password_changed_at,
        }
    }
}

const USER_LIST_QUERY: &str = r#"
    select
        u.id, u.full_name, u.email, u.mobile, u.is_active, u.branch_id,
        b.name as branch_name,
        coalesce((select array_agg(ub.branch_id) from user_branches ub where ub.user_id = u.id), '{}') as branch_ids,
        (select count(*) from user_branches ub where ub.user_id = u.id) as branch_count,
        rr.role_id, rr.role_name,
        u.last_login_at, u.created_at,
        u.must_change_password, u.password_changed_at
    from users u
    left join branches b on b.id = u.branch_id
    left join lateral (
        select ra.role_id, r.name as role_name
        from role_assignments ra
        join roles r on r.id = ra.role_id
        where ra.user_id = u.id
        order by ra.id
        limit 1
    ) rr on true
    where u.organization_id = $1
"#;

async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<TenantUser>>, AppError> {
    auth.require_permission(PERM_MANAGE_USERS)?;

    let rows: Vec<TenantUserRow> = sqlx::query_as(&format!("{USER_LIST_QUERY} order by u.full_name"))
        .bind(auth.organization_id)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(rows.into_iter().map(TenantUserRow::into_domain).collect()))
}

async fn ensure_user_in_org(state: &AppState, user_id: Uuid, organization_id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from users where id = $1 and organization_id = $2)",
    )
    .bind(user_id)
    .bind(organization_id)
    .fetch_one(&state.db)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

async fn ensure_role_in_org(state: &AppState, role_id: Uuid, organization_id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from roles where id = $1 and organization_id = $2)",
    )
    .bind(role_id)
    .bind(organization_id)
    .fetch_one(&state.db)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::bad_request("Choose a valid role."))
    }
}

/// Dedupes while keeping the first occurrence's position — the first
/// entry becomes the primary branch (`TenantUser::branch_id`'s doc
/// comment), so which one survives a duplicate matters.
fn dedupe_branch_ids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

async fn ensure_branches_in_org(
    state: &AppState,
    branch_ids: &[Uuid],
    organization_id: Uuid,
) -> Result<(), AppError> {
    if branch_ids.is_empty() {
        return Ok(());
    }
    let count: i64 = sqlx::query_scalar(
        "select count(*) from branches where organization_id = $1 and id = any($2)",
    )
    .bind(organization_id)
    .bind(branch_ids)
    .fetch_one(&state.db)
    .await?;
    if count as usize == branch_ids.len() {
        Ok(())
    } else {
        Err(AppError::bad_request("Choose valid branches."))
    }
}

/// Replaces a user's whole `user_branches` set inside the caller's
/// transaction and keeps `users.branch_id` (the "primary branch") in
/// sync with the first entry — `create_user`/`update_user` both need
/// exactly this, differing only in whether there's a previous set to
/// diff for the audit log.
async fn replace_branch_assignments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    branch_ids: &[Uuid],
) -> Result<(), AppError> {
    sqlx::query("delete from user_branches where user_id = $1")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    for (i, branch_id) in branch_ids.iter().enumerate() {
        sqlx::query(
            "insert into user_branches (user_id, branch_id, is_primary) values ($1, $2, $3)",
        )
        .bind(user_id)
        .bind(branch_id)
        .bind(i == 0)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query("update users set branch_id = $1 where id = $2")
        .bind(branch_ids.first())
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn normalize_mobile(mobile: Option<String>) -> Option<String> {
    mobile
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
}

async fn create_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateUserInput>,
) -> Result<Json<TenantUser>, AppError> {
    auth.require_permission(PERM_MANAGE_USERS)?;

    let full_name = input.full_name.trim();
    let email = input.email.trim();
    if full_name.is_empty() {
        return Err(AppError::bad_request("Enter a name."));
    }
    if email.is_empty() {
        return Err(AppError::bad_request("Enter an email."));
    }
    if input.temporary_password.len() < 8 {
        return Err(AppError::bad_request(
            "Temporary password must be at least 8 characters.",
        ));
    }
    ensure_role_in_org(&state, input.role_id, auth.organization_id).await?;
    let branch_ids = dedupe_branch_ids(input.branch_ids);
    ensure_branches_in_org(&state, &branch_ids, auth.organization_id).await?;

    let email_taken: bool =
        sqlx::query_scalar("select exists(select 1 from users where email = $1)")
            .bind(email)
            .fetch_one(&state.db)
            .await?;
    if email_taken {
        return Err(AppError::conflict(
            "An account with that email already exists.",
        ));
    }

    let password_hash =
        hash_password(&input.temporary_password).map_err(|e| AppError::Internal(e.into()))?;
    let mobile = normalize_mobile(input.mobile);
    let expires_at = Utc::now() + Duration::days(TEMP_PASSWORD_TTL_DAYS);

    let mut tx = state.db.begin().await?;

    let user_id: Uuid = sqlx::query_scalar(
        r#"insert into users (
               organization_id, full_name, email, password_hash, mobile,
               must_change_password, temp_password_expires_at
           )
           values ($1, $2, $3, $4, $5, true, $6) returning id"#,
    )
    .bind(auth.organization_id)
    .bind(full_name)
    .bind(email)
    .bind(&password_hash)
    .bind(&mobile)
    .bind(expires_at)
    .fetch_one(&mut *tx)
    .await?;

    replace_branch_assignments(&mut tx, user_id, &branch_ids).await?;

    sqlx::query("insert into role_assignments (user_id, role_id) values ($1, $2)")
        .bind(user_id)
        .bind(input.role_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'user', $3, 'user_created')"#,
    )
    .bind(auth.organization_id)
    .bind(auth.user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let row: TenantUserRow = sqlx::query_as(&format!("{USER_LIST_QUERY} and u.id = $2"))
        .bind(auth.organization_id)
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(row.into_domain()))
}

async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateUserInput>,
) -> Result<Json<TenantUser>, AppError> {
    auth.require_permission(PERM_MANAGE_USERS)?;
    ensure_user_in_org(&state, id, auth.organization_id).await?;

    let full_name = input.full_name.trim();
    let email = input.email.trim();
    if full_name.is_empty() {
        return Err(AppError::bad_request("Enter a name."));
    }
    if email.is_empty() {
        return Err(AppError::bad_request("Enter an email."));
    }
    ensure_role_in_org(&state, input.role_id, auth.organization_id).await?;
    let branch_ids = dedupe_branch_ids(input.branch_ids);
    ensure_branches_in_org(&state, &branch_ids, auth.organization_id).await?;

    let email_taken: bool =
        sqlx::query_scalar("select exists(select 1 from users where email = $1 and id <> $2)")
            .bind(email)
            .bind(id)
            .fetch_one(&state.db)
            .await?;
    if email_taken {
        return Err(AppError::conflict(
            "An account with that email already exists.",
        ));
    }

    let mobile = normalize_mobile(input.mobile);
    let previous_role: Option<(Uuid, String)> = sqlx::query_as(
        r#"select r.id, r.name from role_assignments ra join roles r on r.id = ra.role_id
           where ra.user_id = $1 order by ra.id limit 1"#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let previous_branch_ids: Vec<Uuid> =
        sqlx::query_scalar("select branch_id from user_branches where user_id = $1")
            .bind(id)
            .fetch_all(&state.db)
            .await?;

    let mut tx = state.db.begin().await?;

    sqlx::query("update users set full_name = $1, email = $2, mobile = $3 where id = $4")
        .bind(full_name)
        .bind(email)
        .bind(&mobile)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    let branches_changed = {
        let mut prev = previous_branch_ids.clone();
        let mut next = branch_ids.clone();
        prev.sort();
        next.sort();
        prev != next
    };
    if branches_changed {
        replace_branch_assignments(&mut tx, id, &branch_ids).await?;

        sqlx::query(
            r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action, before_state, after_state)
               values ($1, $2, 'user', $3, 'branch_assignment_changed', $4, $5)"#,
        )
        .bind(auth.organization_id)
        .bind(auth.user_id)
        .bind(id)
        .bind(serde_json::json!({ "branch_ids": previous_branch_ids }))
        .bind(serde_json::json!({ "branch_ids": branch_ids }))
        .execute(&mut *tx)
        .await?;
    }

    let role_changed = previous_role.as_ref().map(|(rid, _)| *rid) != Some(input.role_id);
    if role_changed {
        sqlx::query("delete from role_assignments where user_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("insert into role_assignments (user_id, role_id) values ($1, $2)")
            .bind(id)
            .bind(input.role_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action, before_state, after_state)
               values ($1, $2, 'user', $3, 'role_changed', $4, $5)"#,
        )
        .bind(auth.organization_id)
        .bind(auth.user_id)
        .bind(id)
        .bind(previous_role.map(|(_, name)| serde_json::json!({ "role": name })))
        .bind(serde_json::json!({ "role_id": input.role_id }))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let row: TenantUserRow = sqlx::query_as(&format!("{USER_LIST_QUERY} and u.id = $2"))
        .bind(auth.organization_id)
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(row.into_domain()))
}

async fn set_active(
    state: &AppState,
    auth: &AuthUser,
    id: Uuid,
    active: bool,
) -> Result<TenantUser, AppError> {
    auth.require_permission(PERM_MANAGE_USERS)?;
    ensure_user_in_org(state, id, auth.organization_id).await?;

    if !active && id == auth.user_id {
        return Err(AppError::bad_request("You can't deactivate your own account."));
    }

    // Deactivating someone shouldn't leave their already-issued session
    // token working until it expires on its own (up to 24h) — the same
    // `session_valid_after` mechanism a password reset uses.
    sqlx::query("update users set is_active = $1, session_valid_after = now() where id = $2")
        .bind(active)
        .bind(id)
        .execute(&state.db)
        .await?;

    let action = if active { "user_activated" } else { "user_deactivated" };
    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'user', $3, $4)"#,
    )
    .bind(auth.organization_id)
    .bind(auth.user_id)
    .bind(id)
    .bind(action)
    .execute(&state.db)
    .await;

    let row: TenantUserRow = sqlx::query_as(&format!("{USER_LIST_QUERY} and u.id = $2"))
        .bind(auth.organization_id)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(row.into_domain())
}

async fn activate_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TenantUser>, AppError> {
    Ok(Json(set_active(&state, &auth, id, true).await?))
}

async fn deactivate_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TenantUser>, AppError> {
    Ok(Json(set_active(&state, &auth, id, false).await?))
}

/// An admin sets a new temporary password for someone else. Flips
/// `must_change_password` so `login` (routes/auth.rs) won't let them
/// past it without changing it, sets a `TEMP_PASSWORD_TTL_DAYS` expiry,
/// and bumps `session_valid_after` so whatever session they were
/// already in stops working on its next request.
async fn reset_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<ResetPasswordInput>,
) -> Result<Json<TenantUser>, AppError> {
    auth.require_permission(PERM_RESET_PASSWORD)?;
    ensure_user_in_org(&state, id, auth.organization_id).await?;

    if input.temporary_password.len() < 8 {
        return Err(AppError::bad_request(
            "Temporary password must be at least 8 characters.",
        ));
    }
    let password_hash =
        hash_password(&input.temporary_password).map_err(|e| AppError::Internal(e.into()))?;
    let expires_at = Utc::now() + Duration::days(TEMP_PASSWORD_TTL_DAYS);

    sqlx::query(
        r#"update users set
               password_hash = $1,
               must_change_password = true,
               temp_password_expires_at = $2,
               password_changed_at = now(),
               session_valid_after = now()
           where id = $3"#,
    )
    .bind(&password_hash)
    .bind(expires_at)
    .bind(id)
    .execute(&state.db)
    .await?;

    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'user', $3, 'password_reset_initiated')"#,
    )
    .bind(auth.organization_id)
    .bind(auth.user_id)
    .bind(id)
    .execute(&state.db)
    .await;

    let row: TenantUserRow = sqlx::query_as(&format!("{USER_LIST_QUERY} and u.id = $2"))
        .bind(auth.organization_id)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(row.into_domain()))
}

/// Invalidates every session token already issued to this user, without
/// touching their password — for "I think this account's session was
/// left open on a shared machine" rather than "this password is
/// compromised" (`reset_password` covers that, and already does this
/// same bump as part of it).
async fn revoke_sessions(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TenantUser>, AppError> {
    auth.require_permission(PERM_MANAGE_SESSIONS)?;
    ensure_user_in_org(&state, id, auth.organization_id).await?;

    sqlx::query("update users set session_valid_after = now() where id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'user', $3, 'session_revoked')"#,
    )
    .bind(auth.organization_id)
    .bind(auth.user_id)
    .bind(id)
    .execute(&state.db)
    .await;

    let row: TenantUserRow = sqlx::query_as(&format!("{USER_LIST_QUERY} and u.id = $2"))
        .bind(auth.organization_id)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(row.into_domain()))
}
