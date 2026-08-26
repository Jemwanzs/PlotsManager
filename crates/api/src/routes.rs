use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use domain::Organization;
use uuid::Uuid;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/organizations", get(list_organizations))
}

/// Mirrors `domain::Organization` but derives `sqlx::FromRow`. Kept separate
/// so the `domain` crate stays free of the `sqlx` dependency (it also
/// compiles to WASM for the frontend, where sqlx does not belong).
#[derive(sqlx::FromRow)]
struct OrganizationRow {
    id: Uuid,
    name: String,
    code: String,
    currency: String,
    created_at: DateTime<Utc>,
}

impl From<OrganizationRow> for Organization {
    fn from(row: OrganizationRow) -> Self {
        Organization {
            id: row.id,
            name: row.name,
            code: row.code,
            currency: row.currency,
            created_at: row.created_at,
        }
    }
}

async fn list_organizations(
    State(state): State<AppState>,
) -> Result<Json<Vec<Organization>>, (axum::http::StatusCode, String)> {
    // Runtime-checked query (not the sqlx::query_as! macro) so the crate
    // compiles without a live DATABASE_URL at build time. Once the schema
    // stabilises, switch to the macro + `cargo sqlx prepare` for compile-time
    // checked queries.
    let rows: Vec<OrganizationRow> = sqlx::query_as(
        r#"SELECT id, name, code, currency, created_at FROM organizations ORDER BY name"#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows.into_iter().map(Organization::from).collect()))
}
