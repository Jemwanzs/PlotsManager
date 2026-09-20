//! Settings → Users & Access → Roles & Permissions. The `roles` table
//! (`database/migrations/0001_init.sql`) and `AuthUser::permissions`
//! (`crates/backend/src/extractors.rs`) already exist — this is the
//! first place anything lets an admin actually manage what's in it
//! instead of every org running on the single auto-provisioned,
//! everything-permitted "Admin" role from signup.

use axum::extract::Path;
use axum::{extract::State, routing::get, Json, Router};
use domain::{
    all_permission_keys, all_permissions, CreateRoleInput, PermissionDef, Role, UpdateRoleInput,
    PERM_MANAGE_ROLES,
};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/roles", get(list_roles).post(create_role))
        .route(
            "/api/v1/roles/:id",
            axum::routing::put(update_role).delete(delete_role),
        )
        .route("/api/v1/roles/permissions", get(list_permissions))
}

#[derive(sqlx::FromRow)]
struct RoleRow {
    id: Uuid,
    organization_id: Uuid,
    name: String,
    permissions: JsonValue,
    assigned_user_count: i64,
}

impl RoleRow {
    fn into_domain(self) -> Role {
        let permissions = self
            .permissions
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        Role {
            id: self.id,
            organization_id: self.organization_id,
            name: self.name,
            permissions,
            assigned_user_count: self.assigned_user_count,
        }
    }
}

const ROLE_LIST_QUERY: &str = r#"
    select r.id, r.organization_id, r.name, r.permissions,
        (select count(*) from role_assignments ra where ra.role_id = r.id) as assigned_user_count
    from roles r
    where r.organization_id = $1
    order by r.name
"#;

async fn list_roles(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<Role>>, AppError> {
    auth.require_permission(PERM_MANAGE_ROLES)?;

    let rows: Vec<RoleRow> = sqlx::query_as(ROLE_LIST_QUERY)
        .bind(auth.organization_id)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(rows.into_iter().map(RoleRow::into_domain).collect()))
}

/// The catalog itself — static, not org data, but gated the same as the
/// rest of this router so only someone who can reach the Roles editor
/// sees what's available to grant. Returned as the full `PermissionDef`
/// (not just key/label) so the frontend can group by module/feature and
/// flag sensitive ones without a second lookup table.
async fn list_permissions(auth: AuthUser) -> Result<Json<Vec<PermissionDef>>, AppError> {
    auth.require_permission(PERM_MANAGE_ROLES)?;
    Ok(Json(all_permissions()))
}

fn validate_permissions(permissions: &[String]) -> Result<(), AppError> {
    let known = all_permission_keys();
    for p in permissions {
        if p != "*" && !known.contains(&p.as_str()) {
            return Err(AppError::bad_request(format!("Unknown permission \"{p}\".")));
        }
    }
    Ok(())
}

async fn create_role(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateRoleInput>,
) -> Result<Json<Role>, AppError> {
    auth.require_permission(PERM_MANAGE_ROLES)?;

    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("Enter a role name."));
    }
    validate_permissions(&input.permissions)?;

    let duplicate: bool = sqlx::query_scalar(
        "select exists(select 1 from roles where organization_id = $1 and name = $2)",
    )
    .bind(auth.organization_id)
    .bind(name)
    .fetch_one(&state.db)
    .await?;
    if duplicate {
        return Err(AppError::conflict(format!(
            "A role named \"{name}\" already exists."
        )));
    }

    let permissions_json = serde_json::to_value(&input.permissions).unwrap_or_default();
    let id: Uuid = sqlx::query_scalar(
        "insert into roles (organization_id, name, permissions) values ($1, $2, $3) returning id",
    )
    .bind(auth.organization_id)
    .bind(name)
    .bind(&permissions_json)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(Role {
        id,
        organization_id: auth.organization_id,
        name: name.to_string(),
        permissions: input.permissions,
        assigned_user_count: 0,
    }))
}

async fn ensure_role_in_org(
    state: &AppState,
    role_id: Uuid,
    organization_id: Uuid,
) -> Result<(), AppError> {
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
        Err(AppError::NotFound)
    }
}

async fn update_role(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateRoleInput>,
) -> Result<Json<Role>, AppError> {
    auth.require_permission(PERM_MANAGE_ROLES)?;
    ensure_role_in_org(&state, id, auth.organization_id).await?;

    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("Enter a role name."));
    }
    validate_permissions(&input.permissions)?;

    let duplicate: bool = sqlx::query_scalar(
        "select exists(select 1 from roles where organization_id = $1 and name = $2 and id <> $3)",
    )
    .bind(auth.organization_id)
    .bind(name)
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    if duplicate {
        return Err(AppError::conflict(format!(
            "A role named \"{name}\" already exists."
        )));
    }

    let permissions_json = serde_json::to_value(&input.permissions).unwrap_or_default();
    sqlx::query("update roles set name = $1, permissions = $2 where id = $3")
        .bind(name)
        .bind(&permissions_json)
        .bind(id)
        .execute(&state.db)
        .await?;

    let row: RoleRow = sqlx::query_as(
        r#"select r.id, r.organization_id, r.name, r.permissions,
               (select count(*) from role_assignments ra where ra.role_id = r.id) as assigned_user_count
           from roles r where r.id = $1"#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_domain()))
}

/// Refuses to delete a role that's still assigned to anyone — reassign
/// those users first, rather than silently leaving them with no role
/// (and therefore no permissions at all, per `AuthUser::has_permission`).
async fn delete_role(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, AppError> {
    auth.require_permission(PERM_MANAGE_ROLES)?;
    ensure_role_in_org(&state, id, auth.organization_id).await?;

    let assigned_count: i64 =
        sqlx::query_scalar("select count(*) from role_assignments where role_id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await?;
    if assigned_count > 0 {
        return Err(AppError::conflict(format!(
            "This role is still assigned to {assigned_count} user(s) — reassign them first."
        )));
    }

    sqlx::query("delete from roles where id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(()))
}
