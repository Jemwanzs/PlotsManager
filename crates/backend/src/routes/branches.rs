//! Settings -> Branches. A single tenant can operate multiple branches
//! (Platform -> Tenant -> Branches -> Users); this is where a Tenant
//! Admin adds/edits/activates/deactivates them. Multi-branch *user*
//! assignment lives in `routes/users.rs` (the `user_branches` table
//! this module's rows get referenced from).

use axum::extract::Path;
use axum::{extract::State, routing::get, Json, Router};
use domain::{Branch, CreateBranchInput, UpdateBranchInput, PERM_MANAGE_BRANCHES};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/branches", get(list_branches).post(create_branch))
        .route("/api/v1/branches/:id", axum::routing::put(update_branch))
        .route("/api/v1/branches/:id/activate", axum::routing::put(activate_branch))
        .route("/api/v1/branches/:id/deactivate", axum::routing::put(deactivate_branch))
}

#[derive(sqlx::FromRow)]
struct BranchRow {
    id: Uuid,
    organization_id: Uuid,
    name: String,
    code: String,
    region: Option<String>,
    location: Option<String>,
    contact_name: Option<String>,
    contact_phone: Option<String>,
    manager_id: Option<Uuid>,
    manager_name: Option<String>,
    is_active: bool,
}

impl BranchRow {
    fn into_domain(self) -> Branch {
        Branch {
            id: self.id,
            organization_id: self.organization_id,
            name: self.name,
            code: self.code,
            region: self.region,
            location: self.location,
            contact_name: self.contact_name,
            contact_phone: self.contact_phone,
            manager_id: self.manager_id,
            manager_name: self.manager_name,
            is_active: self.is_active,
        }
    }
}

const BRANCH_QUERY: &str = r#"
    select b.id, b.organization_id, b.name, b.code, b.region, b.location,
        b.contact_name, b.contact_phone, b.manager_id, m.full_name as manager_name,
        b.is_active
    from branches b
    left join users m on m.id = b.manager_id
    where b.organization_id = $1
"#;

async fn list_branches(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<Branch>>, AppError> {
    let rows: Vec<BranchRow> = sqlx::query_as(&format!("{BRANCH_QUERY} order by b.name"))
        .bind(auth.organization_id)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(rows.into_iter().map(BranchRow::into_domain).collect()))
}

async fn ensure_manager_in_org(
    state: &AppState,
    manager_id: Option<Uuid>,
    organization_id: Uuid,
) -> Result<(), AppError> {
    let Some(manager_id) = manager_id else {
        return Ok(());
    };
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from users where id = $1 and organization_id = $2)",
    )
    .bind(manager_id)
    .bind(organization_id)
    .fetch_one(&state.db)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::bad_request("Choose a valid branch manager."))
    }
}

async fn create_branch(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateBranchInput>,
) -> Result<Json<Branch>, AppError> {
    auth.require_permission(PERM_MANAGE_BRANCHES)?;

    let name = input.name.trim();
    let code = input.code.trim().to_uppercase();
    if name.is_empty() {
        return Err(AppError::bad_request("Enter a branch name."));
    }
    if code.is_empty() {
        return Err(AppError::bad_request("Enter a branch code."));
    }
    ensure_manager_in_org(&state, input.manager_id, auth.organization_id).await?;

    let duplicate: bool = sqlx::query_scalar(
        "select exists(select 1 from branches where organization_id = $1 and code = $2)",
    )
    .bind(auth.organization_id)
    .bind(&code)
    .fetch_one(&state.db)
    .await?;
    if duplicate {
        return Err(AppError::conflict(format!(
            "A branch with code \"{code}\" already exists."
        )));
    }

    let region = input.region.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
    let location = input.location.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
    let contact_name = input.contact_name.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
    let contact_phone = input.contact_phone.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());

    let id: Uuid = sqlx::query_scalar(
        r#"insert into branches (organization_id, name, code, region, location, contact_name, contact_phone, manager_id)
           values ($1, $2, $3, $4, $5, $6, $7, $8) returning id"#,
    )
    .bind(auth.organization_id)
    .bind(name)
    .bind(&code)
    .bind(&region)
    .bind(&location)
    .bind(&contact_name)
    .bind(&contact_phone)
    .bind(input.manager_id)
    .fetch_one(&state.db)
    .await?;

    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'branch', $3, 'branch_created')"#,
    )
    .bind(auth.organization_id)
    .bind(auth.user_id)
    .bind(id)
    .execute(&state.db)
    .await;

    let row: BranchRow = sqlx::query_as(&format!("{BRANCH_QUERY} and b.id = $2"))
        .bind(auth.organization_id)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(row.into_domain()))
}

async fn ensure_branch_row_in_org(
    state: &AppState,
    id: Uuid,
    organization_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from branches where id = $1 and organization_id = $2)",
    )
    .bind(id)
    .bind(organization_id)
    .fetch_one(&state.db)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

async fn update_branch(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateBranchInput>,
) -> Result<Json<Branch>, AppError> {
    auth.require_permission(PERM_MANAGE_BRANCHES)?;
    ensure_branch_row_in_org(&state, id, auth.organization_id).await?;

    let name = input.name.trim();
    let code = input.code.trim().to_uppercase();
    if name.is_empty() {
        return Err(AppError::bad_request("Enter a branch name."));
    }
    if code.is_empty() {
        return Err(AppError::bad_request("Enter a branch code."));
    }
    ensure_manager_in_org(&state, input.manager_id, auth.organization_id).await?;

    let duplicate: bool = sqlx::query_scalar(
        "select exists(select 1 from branches where organization_id = $1 and code = $2 and id <> $3)",
    )
    .bind(auth.organization_id)
    .bind(&code)
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    if duplicate {
        return Err(AppError::conflict(format!(
            "A branch with code \"{code}\" already exists."
        )));
    }

    let region = input.region.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
    let location = input.location.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
    let contact_name = input.contact_name.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
    let contact_phone = input.contact_phone.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());

    sqlx::query(
        r#"update branches set name = $1, code = $2, region = $3, location = $4,
               contact_name = $5, contact_phone = $6, manager_id = $7
           where id = $8"#,
    )
    .bind(name)
    .bind(&code)
    .bind(&region)
    .bind(&location)
    .bind(&contact_name)
    .bind(&contact_phone)
    .bind(input.manager_id)
    .bind(id)
    .execute(&state.db)
    .await?;

    let row: BranchRow = sqlx::query_as(&format!("{BRANCH_QUERY} and b.id = $2"))
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
) -> Result<Branch, AppError> {
    auth.require_permission(PERM_MANAGE_BRANCHES)?;
    ensure_branch_row_in_org(state, id, auth.organization_id).await?;

    sqlx::query("update branches set is_active = $1 where id = $2")
        .bind(active)
        .bind(id)
        .execute(&state.db)
        .await?;

    let action = if active { "branch_activated" } else { "branch_deactivated" };
    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action)
           values ($1, $2, 'branch', $3, $4)"#,
    )
    .bind(auth.organization_id)
    .bind(auth.user_id)
    .bind(id)
    .bind(action)
    .execute(&state.db)
    .await;

    let row: BranchRow = sqlx::query_as(&format!("{BRANCH_QUERY} and b.id = $2"))
        .bind(auth.organization_id)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(row.into_domain())
}

async fn activate_branch(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Branch>, AppError> {
    Ok(Json(set_active(&state, &auth, id, true).await?))
}

async fn deactivate_branch(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Branch>, AppError> {
    Ok(Json(set_active(&state, &auth, id, false).await?))
}
