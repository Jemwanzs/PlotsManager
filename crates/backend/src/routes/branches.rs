//! Settings -> Branches. Only a read endpoint exists so far — the
//! Users & Access branch-assignment dropdown needs somewhere to read
//! from; full CRUD (add/edit/activate/deactivate a branch) is a later
//! phase of the same tenant/user/branch spec, added to this same file.

use axum::{extract::State, routing::get, Json, Router};
use domain::Branch;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/branches", get(list_branches))
}

#[derive(sqlx::FromRow)]
struct BranchRow {
    id: Uuid,
    organization_id: Uuid,
    name: String,
    code: String,
    region: Option<String>,
}

impl BranchRow {
    fn into_domain(self) -> Branch {
        Branch {
            id: self.id,
            organization_id: self.organization_id,
            name: self.name,
            code: self.code,
            region: self.region,
        }
    }
}

async fn list_branches(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<Branch>>, AppError> {
    let rows: Vec<BranchRow> = sqlx::query_as(
        "select id, organization_id, name, code, region from branches where organization_id = $1 order by name",
    )
    .bind(auth.organization_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(BranchRow::into_domain).collect()))
}
