//! Settings -> Users & Access. Tenant Admins manage the users belonging
//! to their own organization — never across the tenant boundary (every
//! query here is scoped to `auth.organization_id`, same as the rest of
//! the app). Password reset / temporary-password expiry / session
//! revocation are a later phase of the same spec; this one covers list,
//! create, edit (name/email/mobile/branch/role), and activate/
//! deactivate.

use axum::extract::Path;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use domain::{CreateUserInput, TenantUser, UpdateUserInput, PERM_MANAGE_USERS};
use uuid::Uuid;

use crate::auth::hash_password;
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/users", get(list_users).post(create_user))
        .route("/api/v1/users/:id", axum::routing::put(update_user))
        .route("/api/v1/users/:id/activate", axum::routing::put(activate_user))
        .route("/api/v1/users/:id/deactivate", axum::routing::put(deactivate_user))
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
    role_id: Option<Uuid>,
    role_name: Option<String>,
    last_login_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
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
            role_id: self.role_id,
            role_name: self.role_name,
            last_login_at: self.last_login_at,
            created_at: self.created_at,
        }
    }
}

const USER_LIST_QUERY: &str = r#"
    select
        u.id, u.full_name, u.email, u.mobile, u.is_active, u.branch_id,
        b.name as branch_name,
        rr.role_id, rr.role_name,
        u.last_login_at, u.created_at
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

async fn ensure_branch_in_org(
    state: &AppState,
    branch_id: Option<Uuid>,
    organization_id: Uuid,
) -> Result<(), AppError> {
    let Some(branch_id) = branch_id else {
        return Ok(());
    };
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from branches where id = $1 and organization_id = $2)",
    )
    .bind(branch_id)
    .bind(organization_id)
    .fetch_one(&state.db)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::bad_request("Choose a valid branch."))
    }
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
    ensure_branch_in_org(&state, input.branch_id, auth.organization_id).await?;

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

    let mut tx = state.db.begin().await?;

    let user_id: Uuid = sqlx::query_scalar(
        r#"insert into users (organization_id, branch_id, full_name, email, password_hash, mobile)
           values ($1, $2, $3, $4, $5, $6) returning id"#,
    )
    .bind(auth.organization_id)
    .bind(input.branch_id)
    .bind(full_name)
    .bind(email)
    .bind(&password_hash)
    .bind(&mobile)
    .fetch_one(&mut *tx)
    .await?;

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
    ensure_branch_in_org(&state, input.branch_id, auth.organization_id).await?;

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

    let mut tx = state.db.begin().await?;

    sqlx::query(
        "update users set full_name = $1, email = $2, mobile = $3, branch_id = $4 where id = $5",
    )
    .bind(full_name)
    .bind(email)
    .bind(&mobile)
    .bind(input.branch_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

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

    sqlx::query("update users set is_active = $1 where id = $2")
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
